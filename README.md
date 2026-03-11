# SpixelaTUIr

A high-performance, terminal-based image glitching and processing tool written in Rust.

## Features

- **Live preview canvas** — renders processed images directly in the terminal using the Sixel graphics protocol (with ANSI half-block fallback for terminals that don't support Sixel); the status bar always shows the active proxy resolution (e.g. `[512px]`)
- **Interactive effects pipeline** — build a chain of effects that are applied in real-time to a downscaled proxy of your image; the Effects panel title always shows the current effect count (e.g. `Effects (3)`)
- **Multi-threaded** — image processing runs on a dedicated worker thread, keeping the UI responsive at 60 FPS; per-pixel loops are auto-vectorised by the compiler
PNG, JPEG, GIF, BMP
- **Pipeline randomisation** — instantly randomise all effect parameters with a single keypress
- **Pipeline save / load** — export your favourite pipeline to a JSON file and re-import it in any future session
- **Undo / redo** — up to 20 levels of pipeline undo (`Ctrl+Z`) and redo (`Ctrl+Y`)
- **Unsaved-changes guard** — a confirmation prompt prevents accidentally quitting with an unsaved pipeline

## Effects

| Category | Effect | Description |
|----------|--------|-------------|
| **Color** | `HueShift` | Rotate the colour spectrum by N degrees (HSL) |
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
| `Tab` | Toggle keyboard focus between Canvas and Effects panel |
| `↑` / `k` | Navigate effect list up (requires Effects panel focus) |
| `↓` / `j` | Navigate effect list down (requires Effects panel focus) |
| `a` | Add an effect from the preset menu (requires Effects panel focus) |
| `d` / `Delete` | Delete the selected effect and re-process (requires Effects panel focus) |
| `Enter` | Edit parameters of the selected effect (requires Effects panel focus) |
| `K` / `Shift+↑` | Move selected effect one position up in the pipeline (cyan highlight while dragging) |
| `J` / `Shift+↓` | Move selected effect one position down in the pipeline (cyan highlight while dragging) |
| `r` | Randomise all effect parameter values |
| `e` | Export the current preview as an image (dialog with directory/filename/format) |
| `[` | Decrease preview resolution tier (1024 → 768 → 512 → 256 px) |
| `]` | Increase preview resolution tier (256 → 512 → 768 → 1024 px) |
| `Ctrl+S` | Save the current pipeline via a dialog (always writes JSON) |
| `Ctrl+L` | Load / import a pipeline from a JSON or YAML file (file browser) |
| `Ctrl+D` | Clear all effects at once (shows a confirmation prompt) |
| `Ctrl+Z` | Undo the last pipeline change (up to 20 levels) |
| `Ctrl+Y` | Redo the last undone pipeline change |
| `h` | Open the full keyboard-shortcut help overlay |
| `q` / `Esc` | Quit (prompts to confirm if there are unsaved pipeline changes; press `q` again to force-quit) |

## Building

Requires Rust (stable) and `libchafa-dev` (for the Chafa-backed image renderer):

```bash
# Ubuntu / Debian
sudo apt-get install libchafa-dev libglib2.0-dev

# macOS (Homebrew)
brew install chafa

cargo build --release
cargo run --release
```

The `.cargo/config.toml` in this repository sets `PKG_CONFIG_PATH` for Linux so that `libchafa` is found automatically after the packages above are installed.

## Architecture

```
Main Thread (UI)          Engine Thread (Worker)
─────────────────         ──────────────────────
ratatui + crossterm  ──WorkerCommand──▶  pipeline.apply_image()
                                         image::open()
Sixel / half-block  ◀─WorkerResponse─   pipeline.apply_image()
canvas rendering         ProcessedFrame
                         Exported
                         Error
```

- `src/app.rs` — `AppState`, event loop, key handling, randomisation engine  
- `src/ui/` — layout, canvas (Sixel), effects sidebar, widget overlays  
- `src/effects/` — per-effect math (`color`, `glitch`, `crt`, `composite`)  
- `src/engine/` — worker thread, PNG export  
- `src/config/` — YAML / JSON pipeline serialisation  

## Image Formats

Supported via the [`image`](https://github.com/image-rs/image) crate: **PNG, JPEG, GIF, BMP**.

## License

See [LICENSE](LICENSE).
