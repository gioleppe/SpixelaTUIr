# Non-Destructive Masking System — Design Specification

## 1. Overview

This document specifies the design for an **image masking system** that
allows users to apply effects selectively to specific regions of an image.
A mask is a grayscale intensity map where white (255) means "full effect"
and black (0) means "no effect" (original pixels preserved). Intermediate
values produce a smooth blend between the effected and original pixels.

Masks can be loaded from external grayscale images, painted interactively
using a keyboard-driven brush, or generated procedurally (radial gradient,
linear gradient, edge-detection auto-mask).

---

## 2. Motivation

| Limitation | Desired Outcome |
|------------|-----------------|
| Effects apply uniformly to the entire image | Glitch only the sky, leave the subject untouched |
| Vignette is the only spatially-varying effect | Any effect should support spatial variation |
| No way to protect regions during randomisation | Mask out a face before randomising the pipeline |
| Creative limitation for compositing | Combine different effect chains on different regions without layers |

---

## 3. Core Mental Model

> **A mask is a per-pixel multiplier on effect strength.**

```
Original pixel  ─────┐
                      │
Pipeline output ──┐   │
                  ▼   ▼
             mask_value = mask[x, y] / 255.0
             final = lerp(original, effected, mask_value)
```

Masks are applied **per-effect** (each `EnabledEffect` can have its own
optional mask) or **per-pipeline** (a single mask for the entire chain).

---

## 4. Data Structures

### 4.1 Mask

```rust
/// A grayscale mask image. Stored as a single-channel u8 buffer.
#[derive(Clone, Debug)]
pub struct Mask {
    /// Pixel intensities (0 = no effect, 255 = full effect).
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl Mask {
    /// Create a fully white (pass-through) mask.
    pub fn full(width: u32, height: u32) -> Self { ... }

    /// Create a fully black (block-all) mask.
    pub fn empty(width: u32, height: u32) -> Self { ... }

    /// Sample the mask at (x, y). Returns 0.0–1.0.
    /// Coordinates outside bounds return 0.0 (no effect applied), which is
    /// the safe default: out-of-bounds pixels are left untouched by the effect.
    pub fn sample(&self, x: u32, y: u32) -> f32 {
        if x >= self.width || y >= self.height { return 0.0; }
        self.data[(y * self.width + x) as usize] as f32 / 255.0
    }

    /// Resize the mask to match a new image dimension (bilinear interpolation).
    pub fn resize(&self, new_width: u32, new_height: u32) -> Self { ... }
}
```

### 4.2 MaskSource

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MaskSource {
    /// Loaded from an external grayscale image file.
    File(PathBuf),
    /// Procedurally generated radial gradient (center out).
    RadialGradient { center_x: f32, center_y: f32, radius: f32, invert: bool },
    /// Procedurally generated linear gradient.
    LinearGradient { angle: f32, offset: f32, invert: bool },
    /// Auto-generated from edge detection (Sobel) of the source image.
    EdgeDetect { threshold: f32, blur_radius: f32 },
    /// Luminance threshold: pixels above threshold are masked.
    LuminanceThreshold { threshold: f32, invert: bool },
    /// Hand-painted via the brush tool (stored as raw bytes, not serialised).
    Painted,
}
```

### 4.3 EnabledEffect Changes

```rust
// Extended EnabledEffect:
pub struct EnabledEffect {
    pub enabled: bool,
    pub effect: Effect,
    /// Optional per-effect mask. None = apply to entire image.
    pub mask: Option<MaskSource>,
    /// Runtime-resolved mask data (populated from mask source).
    #[serde(skip)]
    pub resolved_mask: Option<Mask>,
}
```

### 4.4 AppState Additions

```rust
// In AppState:
/// Global pipeline mask (applies to all effects collectively).
pub global_mask: Option<MaskSource>,
pub global_resolved_mask: Option<Mask>,
/// Brush state for interactive mask painting.
pub mask_brush: MaskBrushState,
```

```rust
#[derive(Clone, Debug, Default)]
pub struct MaskBrushState {
    /// Brush position in image coordinates.
    pub x: u32,
    pub y: u32,
    /// Brush radius in pixels.
    pub radius: u32,
    /// Brush intensity: 0 = erase, 255 = paint.
    pub intensity: u8,
    /// Whether the brush is actively painting.
    pub painting: bool,
}
```

---

## 5. Mask Application in the Pipeline

### 5.1 Per-Effect Masking

When an effect has a mask, the engine applies it as follows:

```rust
fn apply_effect_masked(
    image: &mut DynamicImage,
    effect: &Effect,
    mask: &Mask,
) {
    // 1. Clone the image before applying the effect.
    let original = image.clone();

    // 2. Apply the effect to the full image.
    effect.apply_image(image);

    // 3. Blend original and effected pixels using the mask.
    let pixels = image.as_mut_rgba8().unwrap();
    let orig_pixels = original.as_rgba8().unwrap();

    for (x, y, px) in pixels.enumerate_pixels_mut() {
        let t = mask.sample(x, y);
        let orig = orig_pixels.get_pixel(x, y);
        for c in 0..3 {
            px[c] = ((orig[c] as f32) * (1.0 - t) + (px[c] as f32) * t) as u8;
        }
    }
}
```

### 5.2 Global Pipeline Masking

The global mask is applied once after the entire pipeline:

```rust
fn apply_pipeline_with_global_mask(
    original: &DynamicImage,
    pipeline: &Pipeline,
    global_mask: &Mask,
) -> DynamicImage {
    let mut output = original.clone();
    pipeline.apply(&mut output);  // apply all effects

    // Blend original and pipeline output using global mask
    blend_with_mask(&original, &mut output, global_mask);
    output
}
```

### 5.3 Performance Optimisation

The `clone()` before each masked effect is expensive. Optimisations:

1. **Skip clone when mask is fully white:** If `mask.data.iter().all(|&v| v == 255)`,
   apply the effect normally without cloning.
2. **Skip effect when mask is fully black:** If `mask.data.iter().all(|&v| v == 0)`,
   skip the effect entirely.
3. **Lazy mask resolution:** Masks are resolved (loaded/generated) only once
   and cached in `resolved_mask`. They are re-resolved only when the source
   changes or the image dimensions change.
4. **Parallel blending:** The mask blend step uses `rayon::par_iter_mut()`
   for the pixel loop.

---

## 6. TUI Layout & Brush Painting

### 6.1 Mask Overlay on Canvas

When mask editing mode is active, the canvas shows a semi-transparent red
overlay where the mask value is 0 (no effect), helping the user visualise
which regions are masked:

```
┌───────────────────────────────────┐
│  Canvas                           │
│                                   │
│   ████████░░░░░░░░░░████████     │  (red overlay = masked out)
│   ████████░░░░░░░░░░████████     │
│   ████████░░░░░░░░░░████████     │
│   ░░░░░░░░░░░░░░░░░░░░░░░░░     │  (clear = effect applied)
│   ░░░░░░░░░░░░░░░░░░░░░░░░░     │
│                                   │
│           [+] ← brush cursor     │
│                                   │
└───────────────────────────────────┘
```

### 6.2 Brush Cursor

The brush is a crosshair `+` rendered at the brush position. The canvas
maps terminal cell coordinates to image pixel coordinates using the known
proxy scale factor.

---

## 7. Keyboard Shortcuts

### 7.1 Normal Mode

| Key | Action |
|-----|--------|
| `M` | Toggle mask editing mode for the selected effect |
| `Shift+M` | Open global mask picker (File / Radial / Linear / Edge / Luminance / Clear) |

### 7.2 Mask Editing Mode (InputMode::MaskEdit)

| Key | Action |
|-----|--------|
| `Arrow keys` / `h/j/k/l` | Move brush cursor |
| `Shift + Arrow` | Move brush cursor by 10 pixels |
| `Space` | Toggle paint / erase mode |
| `p` | Start/stop painting (hold to paint continuously) |
| `+` / `-` | Increase / decrease brush radius |
| `[` / `]` | Decrease / increase brush intensity (softness) |
| `i` | Invert the entire mask |
| `f` | Fill the entire mask (white = pass-through) |
| `c` | Clear the entire mask (black = block all) |
| `Enter` | Confirm mask and return to Normal mode |
| `Esc` | Discard mask changes and return to Normal mode |

### 7.3 Mask Source Picker Dialog

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate options |
| `Enter` | Select mask source type |
| `Esc` | Cancel |

Options:

1. **Load from file** — opens file browser filtered to `.png`, `.jpg`, `.bmp`
2. **Radial gradient** — numeric inputs: center X/Y, radius
3. **Linear gradient** — numeric inputs: angle, offset
4. **Edge detection** — numeric inputs: threshold, blur radius
5. **Luminance threshold** — numeric input: threshold, invert toggle
6. **Paint interactively** — enters brush mode
7. **Clear mask** — removes the mask (full image affected)

---

## 8. Pipeline Persistence

Masks are saved alongside the pipeline in the composition JSON:

```json
{
  "effects": [
    {
      "enabled": true,
      "effect": { "type": "HueShift", "degrees": 90.0 },
      "mask": {
        "type": "RadialGradient",
        "center_x": 0.5,
        "center_y": 0.5,
        "radius": 0.7,
        "invert": false
      }
    },
    {
      "enabled": true,
      "effect": { "type": "Invert" },
      "mask": null
    }
  ],
  "global_mask": {
    "type": "LuminanceThreshold",
    "threshold": 0.5,
    "invert": true
  }
}
```

Painted masks are **not** serialised into the pipeline JSON (they are
ephemeral). To persist a painted mask, the user must export it as a PNG
and then reference it as a `File` mask source.

---

## 9. Worker Thread Changes

The worker thread needs to:

1. Resolve masks (load files, generate procedural masks) when a pipeline is
   received for processing.
2. Apply per-effect masking during pipeline execution.
3. Apply global masking after the pipeline.

```rust
// New WorkerCommand variant:
WorkerCommand::ResolveMask {
    source: MaskSource,
    width: u32,
    height: u32,
    response_tx: Sender<WorkerResponse>,
},

// New WorkerResponse variant:
WorkerResponse::MaskResolved {
    mask: Mask,
},
```

---

## 10. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| Mask file not found | Status bar: `"⚠ Mask file not found: path.png"`; effect applied without mask |
| Mask dimensions mismatch | Auto-resize mask to match image dimensions (bilinear) |
| Mask file is not grayscale | Convert to grayscale (luminance) automatically |
| Brush position out of bounds | Clamp to image bounds silently |
| Painted mask on export (full res) | Upscale painted mask from proxy resolution to source resolution |

---

## 11. File Changes Summary

| File | Change |
|------|--------|
| `src/effects/mask.rs` *(new)* | `Mask`, `MaskSource`, `MaskBrushState` structs; blend/resolve functions |
| `src/effects/mod.rs` | Import mask module; modify pipeline execution to support masks |
| `src/app/state.rs` | Add `global_mask`, `mask_brush` fields |
| `src/app/handlers.rs` | Add `MaskEdit` input mode handlers; `M`/`Shift+M` bindings |
| `src/app/dialogs.rs` | Add `MaskSourcePickerDialog` |
| `src/engine/worker.rs` | Handle `ResolveMask`; integrate masking into pipeline processing |
| `src/ui/canvas.rs` | Render mask overlay (red tint) and brush cursor |
| `src/ui/widgets.rs` | Mask indicator in effects panel (🎭 icon next to masked effects) |
| `src/config/parser.rs` | Serialize/deserialize `MaskSource` in pipeline JSON |
| `README.md` | Document masking feature, keyboard shortcuts |

---

## 12. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Data model** | `Mask`, `MaskSource`, `EnabledEffect.mask` field | Compiling types, serialisation tests |
| **2 — Procedural masks** | Radial gradient, linear gradient, luminance threshold generators | Masks generated at runtime |
| **3 — Per-effect masking** | Pipeline executor applies effect + mask blend | Masked effects work end-to-end |
| **4 — Global masking** | Post-pipeline global mask application | Full masking pipeline |
| **5 — Mask picker UI** | Dialog for selecting mask source type + parameters | User can attach masks via UI |
| **6 — File masks** | Load grayscale images as masks via file browser | External mask support |
| **7 — Brush painting** | `MaskEdit` input mode, brush cursor, canvas overlay | Interactive mask creation |
| **8 — Edge detection** | Auto-mask from Sobel edge detection of source image | Smart masking |
| **9 — Persistence** | Save/load masks in pipeline JSON | Reloadable masked pipelines |

---

## 13. Open Questions

1. **Mask resolution:** Should painted masks be stored at proxy resolution
   (fast, lossy on export) or at full source resolution (slow painting,
   pixel-perfect export)? Recommendation: proxy resolution for painting,
   with bilinear upscaling at export time.

2. **Feathering:** Should mask edges support configurable feathering (Gaussian
   blur on the mask itself)? Recommendation: yes — add an optional
   `feather_radius: f32` to `MaskSource`.

3. **Mask inversion at the effect level:** Should each effect's mask have an
   `invert` toggle so the user can quickly flip which region gets the effect?
   Recommendation: yes — add `invert: bool` to `EnabledEffect.mask`.

4. **Multiple masks per effect:** Should effects support multiple masks
   combined (e.g., radial AND luminance)? Recommendation: defer — a single
   mask per effect covers most use cases. Users can multiply masks externally.

5. **Mask preview in effects panel:** Should the effects panel show a small
   icon or thumbnail of the mask next to each masked effect? Recommendation:
   a simple 🎭 icon is sufficient; thumbnails can be added later.
