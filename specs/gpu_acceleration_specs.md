# GPU-Accelerated Effects Pipeline — Design Specification

## 1. Overview

This document specifies the design and implementation plan for adding an
optional **GPU compute backend** to Spix's effect pipeline. The goal is to
offload pixel-heavy operations (per-pixel colour transforms, convolutions,
sorting) to the GPU via `wgpu` compute shaders, achieving 5–50× speedups on
large images while keeping the CPU-only path as the zero-dependency fallback.

The GPU backend is transparent to the user: Spix detects a compatible GPU at
startup, enables the backend automatically, and falls back to the existing
`rayon`-based CPU path if no GPU is available.

---

## 2. Motivation

| Pain Point | Impact |
|------------|--------|
| Full-resolution export of complex pipelines (8+ effects) on a 4K image takes 3–10 s on CPU | Users wait during export; creative flow is interrupted |
| Heavy effects like `DelaunayTriangulation`, `PixelSort`, and `FractalJulia` are compute-bound | Proxy resolution must stay low (≤768 px) to keep preview responsive |
| `rayon` parallelism is limited by CPU core count (typically 4–16 threads) | GPU offers thousands of execution units for embarrassingly parallel pixel math |
| Future effects (neural style transfer, frequency-domain filters) will need more compute | A GPU abstraction future-proofs the engine |

---

## 3. Technology Choice: `wgpu`

| Criterion | `wgpu` | Alternative (`vulkano`, raw Vulkan) |
|-----------|--------|--------------------------------------|
| Cross-platform | ✅ Vulkan, Metal, DX12, WebGPU | Vulkan-only or per-API wrapping |
| Rust-native | ✅ Pure Rust crate | `vulkano` is Rust; raw Vulkan is C FFI |
| Shader language | WGSL (simple, readable) | GLSL/SPIR-V (extra compile step) |
| Compute shaders | ✅ First-class `@compute` support | ✅ |
| Maintenance | Backed by `gfx-rs` team, active ecosystem | Smaller community |
| Binary size overhead | ~2–5 MB (acceptable for a desktop app) | Similar |

**Decision:** Use `wgpu` with WGSL compute shaders.

---

## 4. Architecture

### 4.1 Dual-Backend Engine

```
                    ┌──────────────────────────┐
                    │       WorkerThread        │
                    │                           │
    WorkerCommand──►│  match backend {          │
                    │    GpuBackend => gpu_pipe  │
                    │    CpuBackend => cpu_pipe  │
                    │  }                         │
                    │                           │
                    └──────────┬───────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                                 ▼
    ┌─────────────────┐               ┌─────────────────┐
    │  GpuPipeline     │               │  CpuPipeline     │
    │  (wgpu compute)  │               │  (rayon + image)  │
    └─────────────────┘               └─────────────────┘
```

Both backends implement a shared `PipelineExecutor` trait:

```rust
pub trait PipelineExecutor: Send + Sync {
    /// Apply a full pipeline to an RGBA8 image buffer in-place.
    fn execute(
        &self,
        width: u32,
        height: u32,
        pixels: &mut [u8],   // RGBA8 row-major
        pipeline: &Pipeline,
    ) -> anyhow::Result<()>;

    /// Human-readable backend name for the status bar.
    fn name(&self) -> &'static str;
}
```

### 4.2 GPU Resource Lifecycle

```rust
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    /// Pre-compiled compute pipelines, one per effect type.
    shader_cache: HashMap<&'static str, wgpu::ComputePipeline>,
    /// Staging buffer for CPU ↔ GPU pixel transfers.
    staging_buffer: wgpu::Buffer,
    /// GPU-side image buffer (storage binding).
    image_buffer: wgpu::Buffer,
    /// Current allocated capacity (width × height × 4).
    buffer_capacity: usize,
}
```

`GpuContext` is created once at startup (inside the Worker thread) and reused
across frames. Buffers are reallocated only when the image dimensions change.

### 4.3 Data Flow for a Single Frame

```
1. CPU: DynamicImage → flat &[u8] RGBA8 slice
2. CPU → GPU: queue.write_buffer(image_buffer, pixels)
3. GPU: for each effect in pipeline:
       bind image_buffer + params uniform
       dispatch compute shader (ceil(W*H / 256) workgroups)
4. GPU → CPU: encoder.copy_buffer_to_buffer(image_buffer → staging)
              device.poll(Maintain::Wait)
              staging_buffer.slice(..).map_async(Read)
5. CPU: copy staging slice → DynamicImage
```

Steps 3 is a tight loop of shader dispatches with no intermediate CPU readback
— all effects chain on the same GPU buffer. This avoids per-effect round-trip
overhead.

---

## 5. Shader Design

Each effect is a standalone WGSL compute shader. All shaders share a common
image buffer layout:

```wgsl
@group(0) @binding(0) var<storage, read_write> pixels: array<u32>;
// Each u32 packs one RGBA8 pixel: R in bits [0..7], G [8..15], B [16..23], A [24..31].

struct Params {
    width: u32,
    height: u32,
    // Effect-specific fields follow...
};
@group(0) @binding(1) var<uniform> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.width * params.height) { return; }
    let x = idx % params.width;
    let y = idx / params.width;
    // per-pixel math...
}
```

### 5.1 Effect Shader Mapping (Phase 1 candidates)

| Effect | Shader Complexity | GPU Suitability |
|--------|-------------------|-----------------|
| `Invert` | Trivial (1 – pixel) | ★★★★★ |
| `HueShift` | Simple (RGB→HSV→RGB) | ★★★★★ |
| `Saturation` | Simple | ★★★★★ |
| `Contrast` | Simple | ★★★★★ |
| `ColorQuantization` | Simple (floor math) | ★★★★★ |
| `ChannelSwap` | Trivial (swizzle) | ★★★★★ |
| `Noise` | Simple (RNG per pixel) | ★★★★☆ |
| `Scanlines` | Simple (row-based) | ★★★★☆ |
| `Vignette` | Medium (distance math) | ★★★★☆ |
| `Pixelate` | Medium (block reads) | ★★★☆☆ |
| `RowJitter` | Medium (row-level offset) | ★★★☆☆ |
| `FractalJulia` | Heavy (iterative) | ★★★★★ |
| `PixelSort` | Complex (parallel sort) | ★★☆☆☆ (Phase 2) |
| `DelaunayTriangulation` | Complex (geometry) | ★☆☆☆☆ (stay on CPU) |

### 5.2 Hybrid Execution Strategy

Effects that are poorly suited for GPU (e.g., `DelaunayTriangulation`,
`PixelSort`) remain on the CPU path. The executor detects these and
automatically performs a **GPU→CPU→GPU round-trip** when a CPU-only effect
appears mid-pipeline:

```
Effect 1 (GPU) → Effect 2 (GPU) → Effect 3 (CPU-only) → Effect 4 (GPU)
                                   ↓ readback ↑ upload
```

The engine minimises these round-trips by batching consecutive GPU-compatible
effects into a single dispatch sequence.

---

## 6. AppState & Configuration Changes

### 6.1 New Fields in AppState

```rust
// In AppState or a dedicated EngineConfig:
pub gpu_available: bool,           // Detected at startup
pub gpu_enabled: bool,             // User toggle (default: true if available)
pub gpu_backend_name: String,      // e.g. "Vulkan", "Metal", "DX12"
```

### 6.2 Status Bar Indicator

When GPU is active, the status bar shows:

```
Ready | image.png [512px]                              GPU: Vulkan ⚡
```

When GPU is unavailable or disabled:

```
Ready | image.png [512px]                              CPU: 8 cores
```

### 6.3 Keyboard Shortcut

| Key | Action |
|-----|--------|
| `g` | Toggle GPU acceleration on/off (Normal mode) |

This allows quick A/B comparison of GPU vs CPU rendering for debugging or
when a user experiences GPU driver issues.

---

## 7. Cargo.toml Changes

```toml
[features]
default = ["gpu"]
gpu = ["dep:wgpu", "dep:pollster"]

[dependencies]
wgpu = { version = "24", optional = true }
pollster = { version = "0.4", optional = true }  # block on async GPU init
```

The `gpu` feature is enabled by default but can be disabled for minimal builds:

```sh
cargo build --release --no-default-features   # CPU-only binary
```

---

## 8. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| No GPU adapter found at startup | `gpu_available = false`; status bar shows CPU mode; no error |
| GPU device lost during processing | Catch `wgpu::Error`; fall back to CPU for that frame; show warning in status bar |
| Shader compilation failure | Log error; disable that specific effect on GPU; fall back to CPU for it |
| Buffer allocation failure (OOM) | Fall back to CPU for the entire pipeline; show `"GPU OOM — using CPU"` |
| `--no-default-features` build | All GPU code is conditionally compiled out; zero runtime cost |

---

## 9. Testing Strategy

### 9.1 Unit Tests

- **Pixel parity tests:** For each GPU-ported effect, render a small test image
  (64×64) through both CPU and GPU paths. Assert that the output buffers are
  identical within a ±1 per-channel tolerance (to account for floating-point
  differences between GPU and CPU).
- **Round-trip test:** Upload → download → compare for `GpuContext` buffer ops.

### 9.2 Integration Tests

- **Hybrid pipeline test:** A pipeline with alternating GPU and CPU-only
  effects. Verify correct output and no panics.
- **Fallback test:** Mock a missing GPU adapter and verify the engine
  gracefully uses the CPU path.

### 9.3 Benchmark

- Add `benches/gpu_vs_cpu.rs` using `criterion` to measure per-effect and
  full-pipeline latency on 1024×1024 and 4096×4096 images.

---

## 10. File Changes Summary

| File | Change |
|------|--------|
| `Cargo.toml` | Add `wgpu`, `pollster`; `gpu` feature gate |
| `src/engine/mod.rs` | Add `gpu` module; export `PipelineExecutor` trait |
| `src/engine/gpu.rs` *(new)* | `GpuContext`, `GpuPipeline`, shader loading, buffer management |
| `src/engine/cpu.rs` *(new)* | Extract existing CPU pipeline into `CpuPipeline` impl |
| `src/engine/worker.rs` | Select backend at startup; use `dyn PipelineExecutor` |
| `src/engine/shaders/` *(new dir)* | `.wgsl` files for each GPU-ported effect |
| `src/app/state.rs` | Add `gpu_available`, `gpu_enabled`, `gpu_backend_name` |
| `src/app/handlers.rs` | Add `g` key handler to toggle GPU |
| `src/ui/widgets.rs` | Render GPU indicator in status bar |
| `README.md` | Document GPU feature, `g` shortcut, `--no-default-features` |

---

## 11. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Abstraction** | `PipelineExecutor` trait; extract CPU path into `CpuPipeline`; backend selection in worker | Refactored engine, no functional change |
| **2 — GPU bootstrap** | `GpuContext` init, buffer upload/download, single no-op shader | GPU detected at startup, round-trip verified |
| **3 — Colour effects** | WGSL shaders for Invert, HueShift, Saturation, Contrast, ChannelSwap, ColorQuantization | 6 effects GPU-accelerated |
| **4 — CRT effects** | Scanlines, Noise, Vignette shaders | 3 more effects on GPU |
| **5 — Glitch effects** | Pixelate, RowJitter, FractalJulia, SineWarp shaders | Heavy effects accelerated |
| **6 — Hybrid fallback** | Automatic GPU→CPU→GPU round-trip for CPU-only effects in mixed pipelines | Seamless mixed execution |
| **7 — UX polish** | Status bar GPU indicator, `g` toggle key, benchmark suite | User-facing feature complete |

---

## 12. Open Questions

1. **Async vs blocking GPU init:** `wgpu` adapter/device creation is async.
   Use `pollster::block_on()` in the worker thread at startup, or spin up a
   small Tokio runtime? Recommendation: `pollster` for simplicity — no need
   for a full async runtime just for init.

2. **Shader hot-reload for development:** During development, should shaders be
   loaded from disk (`include_str!` at compile time vs `std::fs::read` at
   runtime)? Recommendation: `include_str!` for release, with a
   `#[cfg(debug_assertions)]` path that reads from disk for rapid iteration.

3. **Shared memory / texture vs storage buffer:** Some effects (e.g.,
   convolutions for `EdgeGlow`) benefit from texture sampling with hardware
   interpolation. Should we use `texture_2d` bindings for read and `storage`
   for write? This adds complexity but improves performance for spatial filters.

4. **WebGPU target:** With `wgpu` supporting WebGPU, a future WASM build of
   Spix could run in the browser. Should the shader architecture anticipate
   this? Recommendation: yes — avoid platform-specific extensions in WGSL.
