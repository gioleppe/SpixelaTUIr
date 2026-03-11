# Suggested Improvements

A collection of easy wins and cool feature ideas for SpixelaTUIr.

---

## Easy Wins

### 1. Undo / Redo for pipeline edits ✅ Done
Add a small history ring-buffer (e.g. last 20 pipeline states) to
`AppState`. Bind `Ctrl+Z` / `Ctrl+Y` to pop/push states.  
This is low-risk because `Pipeline` already derives `Clone`.

### 2. Drag-to-reorder effects ✅ Done
Since the TUI is keyboard-first, a simple improvement is to highlight the
currently "held" effect with a distinct colour while `K`/`J` moves it, so
the user can visually track the drag.

### 3. Preview resolution label in status bar ✅ Done
Show the current proxy resolution (e.g. `[512px]`) next to the image path
in the status bar so the user always knows which tier is active.

### 4. Pipeline validation on load ✅ Done
After `load_pipeline` succeeds, show a brief summary in the status bar:
`"Loaded 4 effects from my_pipeline.json"` instead of just the path.

### 5. Confirm-before-quit when there are unsaved changes ✅ Done
Track a `pipeline_dirty: bool` flag that is set whenever the pipeline is
modified and cleared when it is saved. Show a `"Unsaved changes – press q
again to quit"` prompt on the first quit attempt.

### 6. Keyboard shortcut to clear the pipeline ✅ Done
Bind `Ctrl+D` (or a similar chord) in the Effects panel to clear all effects
at once, with a confirmation prompt.

### 7. Show effect count in the Effects panel title ✅ Done
Change the panel border title from `"Effects"` to `"Effects (3)"` so the
user can see at a glance how many effects are stacked without counting rows.

---

## Cool New Features

### 8. Live histogram overlay ✅ Done
Render a compact luminance / RGB histogram as a small ASCII-art widget in a
corner of the canvas area.  Uses the already-available `preview_buffer`
pixel data – no extra processing thread needed.

### 9. Named pipeline presets / bookmarks
Add a `presets/` directory to the user's config folder (`~/.config/spixelatuir/presets/`)
and expose a quick-pick menu (similar to `AddEffect`) for loading common
pipeline combinations with a single keypress.

### 10. Animation / GIF export
Extend the export pipeline to iterate over a range of a single parameter
(e.g. hue rotation 0°→360°) and assemble frames into an animated GIF or
WebP.  The `image` crate supports multi-frame encoding out of the box.

### 11. Side-by-side before/after split view ✅ Done
Divide the canvas horizontally: the left half renders the original proxy
image and the right half renders the processed preview.  Toggle with a
hotkey (e.g. `v`).

### 12. Per-effect enable/disable toggle ✅ Done
Add a `enabled: bool` field to each `Effect` variant (or wrap effects in an
`EnabledEffect { enabled, effect }` struct).  Bind `Space` in the Effects
panel to toggle the selected effect on/off without removing it from the
pipeline, so the user can A/B compare quickly.

### 13. Gradient-map colour-grading effect ✅ Done
Add a new `ColorEffect::GradientMap { stops: Vec<(f32, [u8;3])> }` that
remaps luminance to a custom colour gradient (e.g. sepia, duotone,
synthwave).

### 14. Pipeline sharing via QR code
Serialize the pipeline to a compact JSON string, compress it, base64-encode
it, and render it as a QR code using the `qrcode` crate directly in the
terminal.  Scan with a phone to share pipelines instantly.

### 15. Batch-process mode (CLI)
Add a `--batch <glob> --pipeline <file> --outdir <dir>` CLI mode that
applies a saved pipeline to multiple images without opening the TUI.
Leverage `rayon` for parallel batch processing.

### 16. Sixel palette auto-tuning
Query the terminal's palette size (via `XTGETTCAP` / `XTSMGRAPHICS`) and
dynamically select the optimal number of Sixel colours to maximise
perceived quality while staying within terminal limits.

### 17. Plugin / scripting support via WASM
Load user-written WebAssembly modules as custom effect nodes.  The module
exports a single `apply(width, height, pixels: &mut [u8])` function.  This
keeps the core binary small while allowing unlimited extensibility.
