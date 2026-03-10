# Rust & SWE Best Practices for SpixelaTUIr

You are an expert Rust developer and AI coding assistant working on "SpixelaTUIr", a high-performance, TUI-based image glitching and processing application. Adhere to the following guidelines strictly when writing, refactoring, or suggesting code for this repository.

## 1. Core Architecture & State Management
* **Strict Decoupling:** Never perform image processing math inside the `ratatui` UI drawing functions. The UI loop must be as lightweight as possible to maintain 60 FPS.
* **Message Passing:** Communication between the Main Thread (UI) and the Engine Thread (Worker) must be handled via `std::sync::mpsc` channels. 
    * UI sends `EngineCommand` enums (e.g., `UpdatePipeline(Pipeline)`, `TriggerRandomization`).
    * Engine sends back processed `DynamicImage` buffers for the UI to render.
* **State Structuring:** Maintain a single source of truth in the UI thread for widget states (sliders, menus), but keep the heavy `DynamicImage` assets (source, proxy, preview buffer, animation frames) managed efficiently, minimizing clones.

## 2. Image Processing & Performance
* **Data Parallelism:** Image processing is highly CPU-bound. Use the `rayon` crate for all heavy pixel iterations. Prefer `.par_iter_mut()` or `.par_chunks_mut()` over standard iterators when applying effects like Pixel Sort, Inversion, or Hue Shifting.
* **Proxy First:** Always perform live preview math on a `proxy_asset` (a `DynamicImage` downscaled to match the terminal's Sixel pixel dimensions). Only apply math to the `source_asset` during the dedicated Export thread workflow.
* **Zero-Cost Abstractions:** When an image passes through the `Pipeline`, mutate a single buffer in-place sequentially where mathematically possible, rather than allocating a new buffer for every `Effect` node in the chain.

## 3. TUI & Rendering (Sixel)
* **Sixel Protocol:** The live preview canvas must be rendered using `ratatui-image` configured strictly with the `ImageProtocol::Sixel` backend to ensure high-fidelity previews across macOS, Linux, and Windows (via Windows Terminal/WezTerm). Provide ANSI half-block rendering only as a graceful fallback.
* **Terminal Restoration:** Never `unwrap()` or `panic!()` blindly. Always implement a custom panic hook that calls `crossterm::terminal::disable_raw_mode()` and leaves the alternate screen before printing the panic trace. Failure to do so will break the user's terminal.

## 4. Error Handling
* **Crates:** Use `anyhow` for application-level error bubbling and general context. Use `thiserror` for defining specific, recoverable library/engine errors (e.g., `ImageLoadError`, `UnsupportedFormat`, `PipelineParseError`).
* **Silent Failures:** Do not silently fail on image I/O or rendering errors. Surface errors to the user via a dedicated error modal or status bar in the TUI.

## 5. Code Style & Tooling
* **Formatting:** Ensure all code adheres to standard `rustfmt`.
* **Linting:** Code must pass `cargo clippy` without warnings. Address clippy suggestions actively.
* **Idiomatic Rust:** Use `match` statements over `if let` when handling complex enums (like the `Effect` enum). Derive standard traits (`Debug`, `Clone`, `Serialize`, `Deserialize`, `Default`) where appropriate.

## 6. Project Structure Adherence
Respect the established directory structure:
* `src/ui/`: Strictly for Ratatui widgets, layouts, and input handling.
* `src/engine/`: Worker thread logic, multi-threading setup, and export sequencing.
* `src/effects/`: Pure mathematical implementations of the image glitches/filters.
* `src/config/`: Serde parsing for loading/saving custom `Pipeline` configurations.