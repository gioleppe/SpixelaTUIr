# Plugin System via WebAssembly — Design Specification

## 1. Overview

This document specifies the design for a **user-extensible plugin system**
powered by WebAssembly (WASM). Users can write custom image effects in any
language that compiles to WASM (Rust, C, C++, Go, AssemblyScript), drop the
`.wasm` file into a plugins directory, and Spix will load it as a first-class
effect node in the pipeline — appearing alongside built-in effects in the
"Add Effect" menu.

The WASM sandbox ensures plugins cannot crash the host, access the filesystem,
or introduce memory safety issues.

---

## 2. Motivation

| Limitation | Desired Outcome |
|------------|-----------------|
| Adding new effects requires modifying Spix source code and recompiling | Users create custom effects without touching Spix's codebase |
| Niche or experimental effects don't belong in the core binary | Plugins keep the core small and focused |
| Community sharing of effects requires PRs and releases | Users share `.wasm` files directly |
| No scripting escape hatch for power users | WASM provides a safe, fast scripting layer |

---

## 3. Technology Choice: `wasmtime`

| Criterion | `wasmtime` | Alternative (`wasmer`, `wasm3`) |
|-----------|------------|----------------------------------|
| Safety | ✅ Memory-safe sandbox, capability-based | ✅ (all WASM runtimes sandbox) |
| Performance | ✅ JIT-compiled, near-native speed | `wasmer` comparable; `wasm3` interpreter (slower) |
| Rust ecosystem | ✅ First-party Rust crate, well-maintained | `wasmer` is also Rust-native |
| WASI support | ✅ Full WASI preview 1 & 2 | Partial |
| Component model | ✅ `wit-bindgen` for typed interfaces | Limited |
| Community | Bytecode Alliance (Mozilla, Fastly, Intel) | Smaller community |

**Decision:** Use `wasmtime` with the Component Model and WIT (WebAssembly
Interface Types) for a typed, versioned plugin API.

---

## 4. Plugin API (WIT Interface)

### 4.1 WIT Definition

```wit
// spix-plugin.wit — the interface every plugin must implement

package spix:plugin@1.0.0;

interface effect {
    /// Metadata about the plugin effect.
    record effect-info {
        /// Display name shown in the Add Effect menu.
        name: string,
        /// Category for menu grouping: "color", "glitch", "crt", "composite", "custom".
        category: string,
        /// Short description shown as a tooltip.
        description: string,
        /// Semantic version of the plugin.
        version: string,
        /// Author name.
        author: string,
    }

    /// A single tunable parameter exposed to the Spix UI.
    record param-descriptor {
        /// Parameter name (e.g., "intensity").
        name: string,
        /// Current value.
        value: float32,
        /// Minimum allowed value.
        min: float32,
        /// Maximum allowed value.
        max: float32,
        /// Step size for the UI slider.
        step: float32,
    }

    /// Return metadata about this effect.
    get-info: func() -> effect-info;

    /// Return the list of tunable parameters.
    get-params: func() -> list<param-descriptor>;

    /// Update parameter values from the host UI.
    /// The list order must match the order returned by get-params.
    set-params: func(params: list<float32>);

    /// Process an image in-place.
    /// `pixels` is a flat RGBA8 buffer (4 bytes per pixel, row-major).
    /// `width` and `height` describe the image dimensions.
    /// The function modifies `pixels` in-place and returns the modified buffer.
    apply: func(width: u32, height: u32, pixels: list<u8>) -> list<u8>;
}

world spix-effect {
    export effect;
}
```

### 4.2 Plugin Developer Workflow

1. Install the Spix Plugin SDK (a small Rust crate or C header):
   ```sh
   cargo add spix-plugin-sdk
   ```

2. Implement the WIT interface:
   ```rust
   // my_plugin/src/lib.rs
   use spix_plugin_sdk::*;

   struct MyEffect {
       intensity: f32,
   }

   impl SpixEffect for MyEffect {
       fn get_info(&self) -> EffectInfo {
           EffectInfo {
               name: "My Custom Glow".into(),
               category: "custom".into(),
               description: "Adds a dreamy glow effect".into(),
               version: "1.0.0".into(),
               author: "username".into(),
           }
       }

       fn get_params(&self) -> Vec<ParamDescriptor> {
           vec![ParamDescriptor {
               name: "intensity".into(),
               value: self.intensity,
               min: 0.0, max: 1.0, step: 0.01,
           }]
       }

       fn set_params(&mut self, params: &[f32]) {
           self.intensity = params[0];
       }

       fn apply(&self, width: u32, height: u32, pixels: &mut [u8]) {
           for chunk in pixels.chunks_exact_mut(4) {
               let r = chunk[0] as f32 / 255.0;
               let g = chunk[1] as f32 / 255.0;
               let b = chunk[2] as f32 / 255.0;
               // Simple bloom: brighten based on intensity
               chunk[0] = ((r + self.intensity * 0.3).min(1.0) * 255.0) as u8;
               chunk[1] = ((g + self.intensity * 0.3).min(1.0) * 255.0) as u8;
               chunk[2] = ((b + self.intensity * 0.3).min(1.0) * 255.0) as u8;
           }
       }
   }
   ```

3. Compile to WASM:
   ```sh
   cargo build --target wasm32-wasip1 --release
   ```

4. Drop the `.wasm` file into `~/.config/spix/plugins/`.

5. Launch Spix → the plugin appears under **Custom** in the Add Effect menu.

---

## 5. Host-Side Architecture

### 5.1 Plugin Loader

```rust
pub struct PluginLoader {
    engine: wasmtime::Engine,
    /// Loaded plugins indexed by their effect name.
    plugins: Vec<LoadedPlugin>,
}

pub struct LoadedPlugin {
    pub info: EffectInfo,
    pub params: Vec<ParamDescriptor>,
    /// Compiled WASM module (can be instantiated per-call).
    module: wasmtime::Module,
    /// Pre-linked component for fast instantiation.
    linker: wasmtime::component::Linker<PluginState>,
}

pub struct PluginState {
    /// Current parameter values (set by the host before each apply call).
    pub params: Vec<f32>,
}
```

### 5.2 Plugin Discovery

At startup, `PluginLoader` scans:

1. `~/.config/spix/plugins/` — user plugins
2. `./plugins/` — project-local plugins (for development)

For each `.wasm` file found:

1. Compile the module (`wasmtime::Module::from_file`).
2. Instantiate temporarily to call `get_info()` and `get_params()`.
3. Store the `LoadedPlugin` with its metadata.
4. Register in `AVAILABLE_EFFECTS` under the plugin's declared category.

### 5.3 Runtime Execution

When a WASM plugin effect is in the pipeline:

```rust
fn apply_wasm_effect(
    plugin: &LoadedPlugin,
    params: &[f32],
    width: u32,
    height: u32,
    pixels: &mut [u8],
) -> anyhow::Result<()> {
    let mut store = wasmtime::Store::new(&plugin.engine, PluginState {
        params: params.to_vec(),
    });

    let instance = plugin.linker.instantiate(&mut store, &plugin.module)?;

    // Call set_params
    let set_params_fn = instance.get_typed_func::<&[f32], ()>(&mut store, "set-params")?;
    set_params_fn.call(&mut store, params)?;

    // Call apply with the pixel buffer
    let apply_fn = instance.get_typed_func::<(u32, u32, &[u8]), Vec<u8>>(
        &mut store, "apply"
    )?;
    let result = apply_fn.call(&mut store, (width, height, pixels))?;

    // Copy result back
    pixels.copy_from_slice(&result);
    Ok(())
}
```

### 5.4 Integration with Effect Enum

```rust
// New variant in Effect enum:
pub enum Effect {
    Color(ColorEffect),
    Glitch(GlitchEffect),
    Crt(CrtEffect),
    Composite(CompositeEffect),
    /// A dynamically loaded WASM plugin effect.
    Plugin(PluginEffect),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEffect {
    /// The plugin's declared name (used to look up the loaded module).
    pub plugin_name: String,
    /// Current parameter values.
    pub params: Vec<f32>,
}
```

---

## 6. Security & Sandboxing

| Concern | Mitigation |
|---------|------------|
| Plugin accesses filesystem | WASM sandbox: no filesystem access by default (no WASI fs capabilities granted) |
| Plugin infinite loop | `wasmtime::Store` configured with fuel/epoch interruption (max 10 billion instructions per call) |
| Plugin excessive memory | `wasmtime::Config` memory limit: 256 MB per plugin instance |
| Plugin crashes | WASM traps are caught as `anyhow::Error`; host continues; status bar shows error |
| Malicious plugin | WASM is memory-safe by design; no access to host memory outside the linear memory |
| Plugin version mismatch | WIT interface is versioned (`@1.0.0`); incompatible plugins show a warning |

### 6.1 Resource Limits Configuration

```rust
let mut config = wasmtime::Config::new();
config.epoch_interruption(true);           // enable timeout
config.max_wasm_stack(1 << 20);            // 1 MB stack limit
config.memory_limit(256 * 1024 * 1024);    // 256 MB memory limit

let engine = wasmtime::Engine::new(&config)?;
// Set epoch deadline: interrupt after 5 seconds of wall time
engine.increment_epoch();
```

---

## 7. TUI Integration

### 7.1 Add Effect Menu

The "Add Effect" menu gains a new tab: **⚡ Plugins**.

```
┌──────────────────────────────────────────┐
│ Add Effect                               │
│                                          │
│ Color │ Glitch │ CRT │ Composite │⚡Plug │
│ ──────────────────────────────────────── │
│ ► My Custom Glow       (username v1.0)   │
│   Retro VHS Effect     (alice v2.1)      │
│   Thermal Vision       (bob v1.3)        │
│                                          │
│ Enter: Add  │  i: Info  │  Esc: Close    │
└──────────────────────────────────────────┘
```

### 7.2 Plugin Info Panel

Pressing `i` on a highlighted plugin shows its full metadata:

```
┌──────────────────────────────────────────┐
│ Plugin: My Custom Glow                   │
│ Author: username                         │
│ Version: 1.0.0                           │
│ Category: custom                         │
│                                          │
│ Adds a dreamy glow effect by             │
│ brightening pixels based on intensity.   │
│                                          │
│ Parameters:                              │
│   intensity  [0.00 ─────────── 1.00]     │
│                                          │
│ Esc: Close                               │
└──────────────────────────────────────────┘
```

### 7.3 Plugin Errors in UI

If a plugin fails during `apply`:

- The effect row shows `[⚠]` instead of `[✓]`.
- Status bar: `"Plugin 'My Custom Glow' error: execution timeout"`.
- The pipeline continues past the failed effect (skip semantics).

---

## 8. Pipeline Persistence

Plugin effects are serialised by name + params:

```json
{
  "effects": [
    {
      "enabled": true,
      "effect": {
        "type": "Plugin",
        "plugin_name": "My Custom Glow",
        "params": [0.75]
      }
    }
  ]
}
```

Loading a pipeline with a plugin that is not installed shows:

```
"⚠ Plugin 'My Custom Glow' not found — effect skipped"
```

---

## 9. Cargo.toml Changes

```toml
[features]
default = ["plugins"]
plugins = ["dep:wasmtime", "dep:wasmtime-wasi"]

[dependencies]
wasmtime = { version = "29", optional = true, default-features = false, features = ["cranelift", "component-model"] }
wasmtime-wasi = { version = "29", optional = true }
```

Feature-gated to keep the binary small for users who don't need plugins:

```sh
cargo build --release --no-default-features   # no plugin support
```

---

## 10. Plugin SDK Crate

A separate crate `spix-plugin-sdk` provides:

1. Rust `#[derive]` macros for implementing the WIT interface.
2. A `build.rs` that generates bindings from the WIT file.
3. Example plugins in a `examples/` directory.
4. A `Makefile` / `cargo-make` task for building to `wasm32-wasip1`.

This crate is published to crates.io independently of the main Spix binary.

---

## 11. File Changes Summary

| File | Change |
|------|--------|
| `Cargo.toml` | Add `wasmtime`, `wasmtime-wasi`; `plugins` feature gate |
| `src/engine/plugin.rs` *(new)* | `PluginLoader`, `LoadedPlugin`, WASM execution logic |
| `src/engine/mod.rs` | Export plugin module |
| `src/effects/mod.rs` | Add `Plugin(PluginEffect)` variant to `Effect` enum |
| `src/effects/plugin.rs` *(new)* | `PluginEffect` struct, `param_descriptors()`, `apply_params()`, `Display` |
| `src/app/pipeline_utils.rs` | Register loaded plugins in `AVAILABLE_EFFECTS` |
| `src/app/state.rs` | Add `plugin_loader: Option<PluginLoader>` |
| `src/app/handlers.rs` | Handle plugin info panel (`i` key in Add Effect menu) |
| `src/ui/effects_panel.rs` | Render `[⚠]` for failed plugins |
| `src/config/parser.rs` | Serialize/deserialize `PluginEffect` |
| `wit/spix-plugin.wit` *(new)* | WIT interface definition |
| `README.md` | Document plugin system, SDK, plugin directory |

---

## 12. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — WIT interface** | Define `spix-plugin.wit`; create `spix-plugin-sdk` crate skeleton | Published WIT + SDK |
| **2 — Plugin loader** | `PluginLoader` scans directories, compiles modules, extracts metadata | Plugins discovered at startup |
| **3 — Runtime execution** | `apply_wasm_effect` function; integration with pipeline executor | Plugins process pixels |
| **4 — Effect enum** | `Plugin(PluginEffect)` variant; `param_descriptors` delegation | Plugins act like built-in effects |
| **5 — UI integration** | ⚡ Plugins tab in Add Effect menu; plugin info panel | User-facing plugin support |
| **6 — Sandboxing** | Fuel limits, memory limits, timeout handling, error display | Safe plugin execution |
| **7 — Persistence** | Save/load pipelines with plugin effects; missing-plugin warnings | Reloadable plugin pipelines |
| **8 — SDK polish** | Example plugins, documentation, crates.io publish | Developer-ready SDK |

---

## 13. Open Questions

1. **Hot-reload:** Should Spix watch the plugins directory for new/changed
   `.wasm` files and reload them without restart? Recommendation: yes (Phase 9)
   — use `notify` crate for filesystem watching.

2. **Plugin marketplace:** Should there be a built-in command to download
   plugins from a URL or registry? Recommendation: defer — file-based
   distribution is sufficient initially.

3. **Multi-pass plugins:** Should plugins be allowed to request multiple
   passes over the image (e.g., blur requires read-then-write)? The current
   API gives the plugin a mutable copy of the pixel buffer, which allows
   internal multi-pass. No API change needed.

4. **GPU plugin support:** Should plugins be able to provide WGSL shaders
   instead of (or in addition to) CPU WASM code? Recommendation: defer to
   a future "GPU plugin" API extension after the GPU backend (see
   `gpu_acceleration_specs.md`) is implemented.

5. **Plugin dependencies:** Should plugins be able to depend on shared WASM
   libraries (e.g., a common math library)? Recommendation: defer — static
   linking into each plugin is simpler and avoids version conflicts.
