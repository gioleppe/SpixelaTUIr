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
- `src/app.rs`: Central `AppState` management, event loop, and keyboard interaction logic.
- `src/engine/`: 
    - `worker.rs`: Dedicated thread for image processing to avoid UI blocking.
    - `export.rs`: Logic for exporting processed frames to disk.
- `src/effects/`: Modular effect implementations categorized into:
    - `color`: Pixel-level operations (Hue, Saturation, Contrast, Inversion, Quantization).
    - `glitch`: Spatial manipulations (Pixelate, Row Jitter, Block Shift, Pixel Sort).
    - `crt`: Retro-display simulations (Scanlines, Noise, Vignette).
    - `composite`: Image layout operations (Crop, Blend).
- `src/ui/`: Ratatui-based rendering components (Canvas, Effects Panel, Modals, Widgets).
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
- **State Management:** `AppState` holds the entire UI and application state. Avoid deep nesting of state where possible.
- **UI:** The UI is modularized. When adding new widgets, place them in `src/ui/` and register them in `src/ui/mod.rs`.

### Adding New Effects
1. Define the effect struct in the appropriate `src/effects/` submodule (e.g., `glitch.rs`).
2. Implement necessary logic in `Effect::apply_image` or `apply_pixel`.
3. Add the effect to the `Effect` enum in `src/effects/mod.rs`.
4. Register the effect in `AVAILABLE_EFFECTS` within `src/app.rs` to make it appear in the "Add Effect" menu.
5. Update `randomize_pipeline` in `src/app.rs` if the effect has parameters.
