# GEMINI.md - SpixelaTUIr

## Project Overview
**SpixelaTUIr** is a high-performance, terminal-based image glitching and processing tool written in Rust. It allows users to apply a real-time chain of effects to images, with a live preview rendered directly in the terminal using the Sixel graphics protocol or ANSI half-block fallback.

### Key Technologies
- **Language:** Rust (Edition 2024)
- **TUI Framework:** [ratatui](https://github.com/ratatui-org/ratatui) with [crossterm](https://github.com/crossterm-rs/crossterm)
- **Image Rendering:** [ratatui-image](https://github.com/ratatui-org/ratatui-image) (supports Sixel, Kitty, and half-blocks)
- **Image Processing:** `image` and `imageproc` crates
- **Serialization:** `serde`, `serde_json`, and `serde_yml` for pipeline save/load
- **Architecture:** Multi-threaded worker-thread model for responsive UI (60 FPS)

## Architecture & Module Structure
- `src/main.rs`: Application entry point, terminal initialization, and panic hooks.
- `src/app/`: Application state, event loop, and keyboard handlers — decomposed into focused sub-modules:
    - `state.rs`: Central `AppState` struct, image loading, undo/redo, worker dispatch, preview management.
    - `handlers.rs`: All keyboard input handlers for every `InputMode` (normal, edit effect, file browser, export dialog, etc.).
    - `file_browser.rs`: `FileBrowserState`, entry types, and directory navigation logic.
    - `dialogs.rs`: `ExportDialogState`, `SavePipelineDialogState`, `InputMode`, `FocusedPanel` enums.
    - `pipeline_utils.rs`: `AVAILABLE_EFFECTS` catalogue, `randomize_pipeline()`, `format_param_value()`.
    - `mod.rs`: Re-exports all public items, contains the `run()` event loop, constants, and tests.
- `src/engine/`: 
    - `worker.rs`: Dedicated thread for image processing to avoid UI blocking.
    - `export.rs`: Logic for exporting processed frames to disk.
- `src/effects/`: Modular effect implementations. Each sub-effect type owns its own `param_descriptors()`, `apply_params()`, `variant_name()`, and `Display` impl — the `Effect` enum in `mod.rs` delegates. Categorized into:
    - `color.rs`: Pixel-level operations (Hue, Saturation, Contrast, Inversion, Quantization, GradientMap).
    - `glitch.rs`: Spatial manipulations (Pixelate, Row Jitter, Block Shift, Pixel Sort).
    - `crt.rs`: Retro-display simulations (Scanlines, Noise, Vignette, Curvature, PhosphorGlow).
    - `composite.rs`: Image layout operations (Crop, Blend).
    - `mod.rs`: `Effect` enum (thin delegation layer), `Pipeline`, `EnabledEffect`, `ParamDescriptor`, and `apply_per_pixel` helper.
- `src/ui/`: Ratatui-based rendering components (Canvas, Effects Panel, Modals, Widgets). No input handling or state mutation.
- `src/config/`: Pipeline serialization logic.

## Building and Running

### System Dependencies
The project requires `libchafa` for high-quality image rendering in the terminal.
- **Ubuntu/Debian:** `sudo apt-get install libchafa-dev libglib2.0-dev`
- **macOS:** `brew install chafa`

### Commands
- **Run:** `cargo run --release`
- **Build:** `cargo build --release`
- **Test:** `cargo test` (includes snapshot tests for UI and pipeline logic)

## Development Conventions

Refer to [`.github/copilot-instructions.md`](.github/copilot-instructions.md) for detailed Rust and software engineering best practices, including decoupling, message passing, and error handling.

### Error Handling
- Uses `anyhow::Result` for application-level errors.
- Uses `thiserror` for defining structured errors in the engine/effects modules.

### Concurrency
- The UI thread communicates with the engine thread via `std::sync::mpsc` channels.
- `WorkerCommand` is used to dispatch processing/export tasks.
- `WorkerResponse` is used to send processed frames back to the UI.

### Coding Style
- **Efficiency:** Tight inner loops in `src/effects/mod.rs` (`apply_per_pixel`) are designed for auto-vectorization by the compiler.
- **State Management:** `AppState` (in `src/app/state.rs`) holds the entire UI and application state. Dialog and file-browser state are encapsulated in dedicated types (`ExportDialogState`, `SavePipelineDialogState`, `FileBrowserState`) within the `app/` module.
- **Effect Ownership:** Each sub-effect type owns its parameter metadata (`param_descriptors()`), parameter mutation (`apply_params()`), display label (`Display` impl), and variant name (`variant_name()`). The `Effect` enum in `mod.rs` is a thin delegation layer.
- **UI:** The UI is modularized. When adding new widgets, place them in `src/ui/` and register them in `src/ui/mod.rs`.

### Adding New Effects
1. Define the effect variant in the appropriate `src/effects/` submodule (e.g., `glitch.rs`).
2. Implement the processing logic (`apply_image` or `apply_pixel`/`apply_pixel_with_coords`).
3. Implement `param_descriptors()`, `apply_params()`, `variant_name()`, and `Display` on the sub-effect type in the same file.
4. Add the variant to the `Effect` enum in `src/effects/mod.rs` (delegation is automatic via the existing match arms).
5. Register the effect in `AVAILABLE_EFFECTS` within `src/app/pipeline_utils.rs` to make it appear in the "Add Effect" menu.
6. Update `randomize_pipeline` in `src/app/pipeline_utils.rs` if the effect has parameters.
