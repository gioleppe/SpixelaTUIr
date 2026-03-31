# Image Glitch & Processing Tool (Spix) - Functional Specification

## 1. Core Architecture & I/O 
The application acts as a real-time (or near real-time) node-based or iterative image processing pipeline, built in Rust to leverage memory safety and data parallelism.



### 1.1 Threading Model
* **Main Thread (UI):** Powered by `ratatui` and `crossterm`. Handles the event loop, terminal resizing, UI state (slider values, active menus), and rendering the `ratatui-image` widgets using the Sixel protocol.
* **Engine Thread (Worker):** Receives state updates from the UI via `std::sync::mpsc`. Processes the downscaled proxy image using `rayon` for data parallelism. Sends the processed frame back to the UI.
* **Export Subsystem:** A dedicated thread spawned specifically for exporting high-resolution stills or rendering animation frames.

### 1.2 I/O & Metadata
* **Image Import:** Support for standard image formats (PNG, JPG, GIF, BMP) via the `image` crate.
* **Live Preview Canvas:** A responsive display that scales the image to the Sixel viewport. Mathematical operations during UI interaction are strictly applied to a downscaled proxy asset.
* **Metadata Readout:** Live TUI display of original dimensions, proxy dimensions, active crop, current pipeline execution time, and preview scale.
* **Export Subsystem:**
  * Still image export (lossless PNG).
  * Animation export (GIF natively, MP4/WebP via FFmpeg bindings or external shell).
  * Palette extraction and export (JSON/HEX).
  * Safe-saving workflow (auto-incrementing filenames).

## 2. Terminal Graphics Protocol
* **Sixel Backend:** To guarantee high-fidelity live previews across macOS, Linux, and Windows (e.g., Windows Terminal, WezTerm), Sixel is the primary image rendering backend via `ratatui-image::ImageProtocol::Sixel`.
* **Graceful Degradation:** Fallback to ANSI half-blocks if the host terminal lacks Sixel support.

## 3. Core Data Structures & State

```rust
pub struct AppState {
    pub source_asset: DynamicImage,           // Full res original
    pub proxy_asset: DynamicImage,            // Downscaled for live Sixel preview
    pub preview_buffer: DynamicImage,         // Current output of the engine
    pub blend_asset: Option<DynamicImage>,    // Secondary image for compositing
    pub animation_frames: Vec<DynamicImage>,  // In-memory sequence frames
    pub pipeline: Pipeline,
    pub crop_bounds: CropRect,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Pipeline {
    pub nodes: Vec<Effect>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CropRect {
    pub top: u32, pub bottom: u32, pub left: u32, pub right: u32,
    pub aspect_lock: Option<f32>,
}
```

## 4. Transform & Glitch Effects
Operations that manipulate the spatial coordinates or structure of the pixel data.

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum Effect {
    /// Resolution downsampling followed by nearest-neighbor upsampling.
    Pixelate { downsample_factor: u32 },
    /// Horizontal spatial displacement of random horizontal pixel rows.
    RowJitter { intensity: f32, probability: f32, seed: u64 },
    /// Moving larger, rectangular chunks of pixels horizontally.
    BlockShift { block_width: u32, block_height: u32, shift_amount: i32 },
    /// Isolating horizontal segments and sorting them by luminance.
    PixelSort { threshold: u8, angle: f32 },
    // ... (continued below)
}
```

## 5. Color & Tonal Manipulation
Mathematical operations performed on the color values of the individual pixels.

```rust
    // ... (Effect enum continued)
    /// Rotation of the global color spectrum (HSV/HSL).
    HueShift { degrees: f32 },
    /// Multiplier for color intensity (0.0 to hyper-saturated).
    Saturation { multiplier: f32 },
    /// Expansion or compression of the distance between darkest/lightest elements.
    Contrast { factor: f32 },
    /// Mathematical inversion of RGB values preserving Alpha.
    Invert,
    /// Reducing the available color palette (posterization).
    ColorQuantization { levels: u8 },
    // ... (continued below)
}
```

## 6. CRT & Analog Emulation (Finishing Effects)
Post-processing filters designed to simulate retro hardware output.

```rust
    // ... (Effect enum continued)
    /// Spherize or bulge distortion to mimic CRT glass.
    Curvature { warp_amount: f32 },
    /// Wavy, sine-wave-like horizontal distortion mimicking analog sync.
    SignalDistortion { frequency: f32, amplitude: f32 },
    /// Bloom effect isolating highlights to simulate light bleed.
    PhosphorGlow { radius: f32, threshold: u8 },
    /// Generation of RGB or monochromatic noise.
    Noise { intensity: f32, monochromatic: bool },
    /// Injection of alternating, semi-transparent dark horizontal lines.
    Scanlines { thickness: u32, opacity: f32 },
    /// Micro-translations of R, G, B channels (Chromatic Aberration).
    RgbShift { r_offset: (i32, i32), g_offset: (i32, i32), b_offset: (i32, i32) },
    /// Radial darkening towards the edges of the canvas.
    Vignette { radius: f32, softness: f32 },
    /// Specific digital degradation (CCD artifacts, lossy JPEG).
    CompressionEmulation { jpeg_quality: u8 },
    // ... (continued below)
}
```

## 7. Composition & Masking
```rust
    // ... (Effect enum continued)
    /// Ability to conform secondary asset dimensions and composite it.
    ImageBlend { blend_mode: BlendMode, intensity: f32 },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum BlendMode { Normal, Multiply, Screen, Overlay }
```

## 8. Geometry & Cropping
* **Four-Edge Crop:** Independent trim values for the top, bottom, left, and right bounds.
* **Aspect Ratio Forcing:** Automatic locking of crop bounds to standard or user-defined aspect ratios (e.g., 1:1, 16:9).

## 9. Animation Workflow
Frame Capture: Captures the current preview_buffer state and pushes it to AppState::animation_frames (held in RAM).

Timeline Strip: TUI visual representation of captured frames (e.g., a horizontally scrollable list of miniature Sixel block renders or text indices).

Sequence Rendering: Stitches the captured frames into a defined frame rate container (GIF/MP4) via the Export Thread.

## 10. Randomization Engine
Global Randomization: An instant TUI trigger (e.g., R key) to randomly populate the numeric values/toggles for all active Effect parameters.

Targeted Randomization: A configuration matrix/menu allowing the user to exclude specific parameters (like crop bounds or specific color shifts) from the global randomization roll.

## 11. Pipeline Execution Logic
The user dictates the order of operations via external configurations (YAML/JSON mapped to the Pipeline struct). If no custom configuration is provided, the standard opinionated top-down pipeline executes as follows:

Geometry/Crop: Establish the bounds.

Transform/Glitch: Alter spatial coordinates.

Color/Tonal: Adjust raw pixel values.

Composition: Blend secondary images.

CRT/Post-Processing: Apply finishing screens and static.

## 12. WASM Plugin System

### 12.1 Overview
Users can extend Spix with custom image effects by supplying WebAssembly (`.wasm`) plugins. Plugins are discovered from `~/.config/spix/plugins/` at startup and integrated into the pipeline as first-class effects alongside built-in ones.

### 12.2 Runtime
* **Engine:** `wasmer` (Cranelift compiler backend, pure-Rust, cross-platform).
* **Registry:** A global `OnceLock<WasmPluginRegistry>` initialized at startup. After initialization, the registry is read-only (no locking).
* **Sandboxing:** Plugins run in isolated WASM linear memory with no WASI imports — they cannot access the filesystem, network, or host APIs.
* **Lifecycle:** Each `process()` call creates a fresh `Store` + `Instance`, ensuring no cross-call memory contamination.

### 12.3 Plugin API Contract
Every `.wasm` plugin must export the following functions:

| Export | Signature | Purpose |
|---|---|---|
| `name` | `() → i32` | Pointer to null-terminated UTF-8 effect name |
| `num_params` | `() → i32` | Number of tunable parameters |
| `param_name` | `(i32) → i32` | Pointer to null-terminated name for param at index |
| `param_default` | `(i32) → f32` | Default value for parameter at index |
| `param_min` | `(i32) → f32` | Minimum value for parameter at index |
| `param_max` | `(i32) → f32` | Maximum value for parameter at index |
| `set_param` | `(i32, f32) → ()` | Set parameter value before processing |
| `process` | `(i32, i32, i32, i32) → i32` | Process RGBA data in-place (width, height, ptr, len). Return 0 on success. |
| `alloc` | `(i32) → i32` | Allocate bytes in WASM linear memory |
| `dealloc` | `(i32, i32) → ()` | Free previously allocated bytes |
| `memory` | *(memory export)* | WASM linear memory |

### 12.4 Memory Protocol
1. Host calls `alloc(width × height × 4)` to get a pointer in WASM memory.
2. Host writes raw RGBA pixel bytes at that pointer.
3. Host calls `set_param(i, value)` for each parameter.
4. Host calls `process(width, height, ptr, len)` — the plugin modifies pixels in-place.
5. Host reads the modified bytes back from the same pointer.
6. Host calls `dealloc(ptr, len)` to free the memory.

### 12.5 Data Structures
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEffect {
    pub plugin: String,       // Plugin name (not file path — portable)
    pub params: Vec<f32>,     // Current parameter values
}
```

### 12.6 Integration Points
* **Effect enum:** `Effect::Wasm(WasmEffect)` variant delegates to `WasmEffect` methods.
* **Pipeline:** WASM effects participate in `Pipeline::apply_image()` like any built-in effect.
* **Serialization:** Automatic via serde derives. Uses plugin name for portability across machines.
* **Add-Effect menu:** WASM plugins appear under a dedicated "WASM" tab (tab index 5).
* **Batch mode:** WASM registry is initialized before batch processing.

### 12.7 Error Handling
| Failure | Behavior |
|---|---|
| `.wasm` fails to compile | Plugin skipped at startup, warning logged |
| Plugin missing required export | Plugin skipped at startup, warning logged |
| `process()` returns non-zero | Image unchanged, error logged |
| WASM trap (OOB memory, etc.) | Image unchanged, error logged |
| Pipeline references missing plugin | Effect passes through unchanged, name still displayed |