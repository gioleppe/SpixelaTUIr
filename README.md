# SpixelaTUIr

![SpixelaTUIr](./readme_assets/pc_on_fire_out.png)


A terminal-based image glitching and processing tool written in Rust.

## What is this thing?

SpixelaTUIr is a terminal-based image glitching and processing application designed for creative tinkering and fast visual experimentation. Built in Rust, it leverages the Sixel graphics protocol to render high-fidelity, live previews of image manipulations entirely inside your terminal emulator.

With SpixelaTUIr, you can intuitively construct and compose complex, multi-layered visual effect pipelines. Combine color manipulation, stylistic retro CRT overlays, and pixel-level structural glitches like pixel sorting, row jittering, and block shifting. It lets you interactively tweak effect parameters, instantly randomize pipelines for unexpected inspiration, and seamlessly visualize your changes in real-time. 


![SpixelaTUIr Example](./readme_assets/spixelatuir_example.png)


Once you've crafted the perfect aesthetic, you can export the full-resolution glitch art to your filesystem or save your custom effect pipeline as a preset to apply to other images.

## Features

- **Live preview canvas** — renders processed images directly in the terminal using the Sixel graphics protocol (with ANSI half-block fallback for terminals that don't support Sixel); the status bar always shows the active proxy resolution (e.g. `[512px]`)
- **Interactive effects pipeline** — build a chain of effects that are applied in real-time to a downscaled proxy of your image; the Effects panel title always shows the current effect count (e.g. `Effects (3)`)
- **Multi-threaded** — image processing runs on a dedicated worker thread, keeping the UI responsive. Re-rendering is only triggered when needed. Per-pixel loops are auto-vectorised by the compiler
- **Animation creation & export** — build multi-frame animations by manually capturing pipeline snapshots (`c`) or using the automatic parameter sweep (`s`) to smoothly interpolate effect values (e.g., HueShift 0→360). Preview animations in-app (`Space`) and export them as animated GIFs or WebP files (`Ctrl+E`)
- **Pipeline randomisation** — instantly randomise all effect parameters with a single keypress
- **Pipeline save / load** — export your favourite pipeline to a JSON file and re-import it in any future session
- **Undo / redo** — up to 20 levels of pipeline undo (`Ctrl+Z`) and redo (`Ctrl+Y`)
- **Unsaved-changes guard** — a prominent centered confirmation modal prevents accidentally quitting with an unsaved pipeline
- **Per-effect enable/disable** — toggle individual effects on/off with `Space` in the Effects panel without removing them, for quick A/B comparisons; disabled effects are shown in grey with a `✗` indicator
- **Side-by-side split view** — press `v` to divide the canvas horizontally, showing the original (before) on the left and the processed preview (after) on the right
- **Live histogram overlay** — press `H` to display a compact luminance histogram in the top-right corner of the canvas, computed from the current preview buffer with no extra processing thread
- **File-browser image preview** — when the "Open Image" file picker is open, a live thumbnail preview of the highlighted image is shown in a right-side panel; the thumbnail loads asynchronously via the worker thread so the UI stays responsive

## Installation

### Installing from source (recommended)
#### Prerequisites

Before building SpixelaTUIr from source, ensure you have the following dependencies installed:

- **Rust (stable)**: Install from [rustup.rs](https://rustup.rs/)

If you are building on Windows, make sure to satisfy the requirements as described here: [Set up your dev environment on Windows for Rust](https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup)

#### Installing with Cargo

To install SpixelaTUIr locally using Cargo, navigate to the project directory and run:

```bash
cargo install --path .
```

This will build the project in release mode and install the `spixelatuir` binary to `~/.cargo/bin`, making it available in your PATH.

### Pre-built binaries

Pre-build binaries are offered in the [Releases page](https://github.com/gioleppe/SpixelaTUIr/releases).
Keep in mind that these might not be tested.

You need a [sixel](https://rioterm.com/docs/features/sixel-protocol)-compatible terminal emulator to run SpixelaTUIr correctly. Windows Terminal (Win) and [Ghostty](https://ghostty.org/) (macOS, Linux) are both good choices. 

## Effects

| Category | Effect | Description |
|----------|--------|-------------|
| **Color** | `HueShift` | Rotate the colour spectrum by N degrees (HSL) |
| | `GradientMap` | Remaps luminance to a custom colour gradient (Synthwave, Sepia, Cyberpunk, Night Vision, or Custom) |
| | `Saturation` | Scale colour intensity (HSL) |
| | `Contrast` | Expand or compress the tonal range |
| | `Invert` | Mathematical RGB inversion |
| | `ColorQuantization` | Posterize to N palette levels |
| **Glitch** | `Pixelate` | Block-average downsampling then nearest-neighbour upscale |
| | `RowJitter` | Deterministic horizontal row displacement |
| | `PixelSort` | Sort above-threshold pixels by luminance within each row |
| | `BlockShift` | Translate the entire image by (x, y) with wrapping |
| **CRT** | `Scanlines` | Semi-transparent dark horizontal lines |
| | `Noise` | Per-pixel RGB or monochromatic noise |
| | `Vignette` | Smooth-step radial edge darkening |
| **Composite** | `CropRect` | Crop to a given rectangle |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `o` | Open an image (file browser) |
| `Tab` | Cycle keyboard focus: Canvas → Effects panel → Animation panel (when open) → Canvas |
| `↑` / `k` | Navigate effect list up (requires Effects panel focus) |
| `↓` / `j` | Navigate effect list down (requires Effects panel focus) |
| `a` | Add an effect from the preset menu (requires Effects panel focus) |
| `d` / `Delete` | Delete the selected effect and re-process (requires Effects panel focus) |
| `Enter` | Edit parameters of the selected effect (requires Effects panel focus). For effects with presets (like `GradientMap`), use `←` / `→` (Left/Right) to cycle between them. |
| `Space` | Toggle the selected effect on/off (requires Effects panel focus); disabled effects are skipped during rendering but stay in the pipeline |
| `K` / `Shift+↑` | Move selected effect one position up in the pipeline (cyan highlight while dragging) |
| `J` / `Shift+↓` | Move selected effect one position down in the pipeline (cyan highlight while dragging) |
| `r` | Randomise all effect parameter values |
| `e` | Export the current preview as an image (dialog with directory/filename/format) |
| `[` | Decrease preview resolution tier (1024 → 768 → 512 → 256 px) |
| `]` | Increase preview resolution tier (256 → 512 → 768 → 1024 px) |
| `v` | Toggle side-by-side before/after split view (left = original, right = processed) |
| `H` | Toggle live luminance histogram overlay in the top-right corner of the canvas |
| `Ctrl+S` | Save the current pipeline via a dialog (always writes JSON) |
| `Ctrl+L` | Load / import a pipeline from a JSON or YAML file (file browser) |
| `Ctrl+D` | Clear all effects at once (shows a confirmation prompt) |
| `Ctrl+Z` | Undo the last pipeline change (up to 20 levels) |
| `Ctrl+Y` | Redo the last undone pipeline change |
| `Ctrl+N` | Toggle the Animation panel open / closed |
| `h` | Open the full keyboard-shortcut help overlay |
| `q` / `Esc` | Quit (shows a confirmation modal if there are unsaved pipeline changes: `y`/Enter to quit, `n`/Esc to cancel, `s` to save pipeline) |

### Animation Panel shortcuts (Tab to focus the animation panel)

| Key | Action |
|-----|--------|
| `←` / `→` | Navigate frames in the timeline |
| `c` | Capture the current pipeline state as a new animation frame |
| `d` / `Delete` | Delete the selected animation frame |
| `Enter` | Load the selected frame's pipeline back into the Effects panel for editing |
| `Space` | Play / pause animation preview |
| `f` | Edit the selected frame's display duration (ms) |
| `F` | Set ALL frames to the same duration (ms) |
| `L` | Toggle loop mode on / off |
| `+` / `-` | Increase / decrease global frame rate (fps) |
| `K` / `Shift+↑` | Move selected frame one position up in the timeline |
| `J` / `Shift+↓` | Move selected frame one position down in the timeline |
| `s` | Open the parameter sweep dialog (auto-generate interpolated frames) |
| `Ctrl+E` | Open the animation export dialog (GIF / WebP) |
| `Esc` | Return focus to the Effects panel |

## Building

Requires Rust (stable):

```bash
cargo build --release
cargo run --release
```

## Architecture

SpixelaTUIr uses a two-thread architecture:

- **Main/UI thread**: terminal lifecycle, event loop, input handling, app state, and rendering (`ratatui` + `ratatui-image`).
- **Worker thread**: CPU-heavy image processing and export operations.

The UI and worker communicate through `std::sync::mpsc` channels using typed messages:

- **UI → Worker**: `WorkerCommand::{Process, Export, RenderAnimationFrame, RenderSweepBatch, ExportAnimation, Quit}`
- **Worker → UI**: `WorkerResponse::{ProcessedFrame, AnimationFrameReady, SweepBatchReady, Exported, Error}`

```mermaid
flowchart LR
  subgraph UI[Main Thread: UI]
    A[main.rs\nterminal setup + panic hook]
    B[app/mod.rs::run\nevent loop at ~16ms poll]
    C[AppState\nsource_asset + proxy_asset + preview_buffer\npipeline + dialogs + selection]
    D[handlers.rs\nkey handling by InputMode]
    E[ui/*\nrender canvas/panels/modals\nSixel with half-block fallback]
  end

  subgraph W[Worker Thread]
    F[engine/worker.rs::run]
    G[pipeline.apply_image\nprocess proxy image]
    H[engine/export.rs\nwrite output file]
  end

  A --> B
  B --> C
  B --> D
  B --> E

  C -- WorkerCommand::Process / RenderAnimationFrame / RenderSweepBatch --> F
  C -- WorkerCommand::Export / ExportAnimation --> F
  C -- WorkerCommand::Quit --> F

  F --> G
  F --> H
  G -- WorkerResponse::ProcessedFrame / AnimationFrameReady / SweepBatchReady --> C
  H -- WorkerResponse::Exported / Error --> C

  C --> E
```

Module ownership:

- `src/main.rs` — terminal setup/restore, panic hook, starts app run loop
- `src/app/mod.rs` — event loop, worker channel wiring, redraw scheduling
- `src/app/state.rs` — central `AppState`, image loading, proxy reload, dispatch to worker, undo/redo
- `src/app/handlers.rs` — keyboard behavior for all modes
- `src/ui/` — pure rendering/layout/widgets (no heavy image math)
- `src/engine/worker.rs` — command handling, stale process draining, process/export dispatch
- `src/engine/export.rs` — format-specific export path
- `src/effects/` — effect math and pipeline execution
- `src/config/` — pipeline load/save (JSON/YAML)

## Image Formats

Supported via the [`image`](https://github.com/image-rs/image) crate: **PNG, JPEG, GIF, BMP**.

## Contributing

This project is developed in the spirit of open collaboration, any meaningful contribution is cherished upon.
Feel free to open PRs and Issues if you want to support new effects.
You can contact me for suggestions, or just to have a chat: you can find some of my handles at https://gioleppe.github.io/

## Disclaimer

SpixelaTUIr is a GenAI driven project. I mainly built it to sharpen my agent-handling skills and to have fun, don't take it too seriously.

## License

See [LICENSE](LICENSE).
