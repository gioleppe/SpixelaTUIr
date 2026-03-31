# Layer & Compositing System — Design Specification

## 1. Overview

This document specifies the design for a **multi-layer compositing system**
in Spix. Currently, Spix operates on a single source image with one
ordered effect pipeline. This feature introduces the concept of **layers** —
independent image sources, each with its own effect pipeline — that are
composited together using configurable blend modes and opacity.

This enables complex creative workflows: glitching different parts of an
image independently, blending multiple source images, or creating text/shape
overlays with per-layer effects.

---

## 2. Motivation

| Limitation | Impact |
|------------|--------|
| Single source image | Cannot combine two differently glitched versions of the same image |
| `ImageBlend` effect is pipeline-global | Blending a secondary image affects the entire pipeline output, not a specific stage |
| No isolation of effect regions | Cannot apply CRT effects to one area and glitch effects to another |
| No generated layers (solid color, gradient, noise) | Must create these externally and load as secondary images |

---

## 3. Core Mental Model

> **A composition is a stack of layers, each with its own pipeline,
> composited from bottom to top.**

```
┌─────────────────────────────────┐
│  Layer 3: "Noise overlay"       │  ← topmost (blends onto result of 1+2)
│  Source: Generated noise         │
│  Pipeline: [HueShift, Invert]   │
│  Blend: Screen, Opacity: 0.4    │
├─────────────────────────────────┤
│  Layer 2: "Glitch copy"         │
│  Source: image.png (same file)   │
│  Pipeline: [PixelSort, RowJitter]│
│  Blend: Normal, Opacity: 0.7    │
├─────────────────────────────────┤
│  Layer 1: "Base"                │  ← bottommost (rendered first)
│  Source: image.png               │
│  Pipeline: [Contrast, Vignette]  │
│  Blend: Normal, Opacity: 1.0    │
└─────────────────────────────────┘
          │
          ▼
   Final composited output
```

---

## 4. Data Structures

### 4.1 Layer

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    /// Unique identifier for this layer.
    pub id: u32,
    /// Display name shown in the Layers panel.
    pub name: String,
    /// Source for this layer's pixel data.
    pub source: LayerSource,
    /// Effect pipeline applied to this layer before compositing.
    pub pipeline: Pipeline,
    /// How this layer blends with the layers below it.
    pub blend_mode: BlendMode,
    /// Opacity for compositing (0.0 = fully transparent, 1.0 = fully opaque).
    pub opacity: f32,
    /// Whether this layer is visible (similar to per-effect enable/disable).
    pub visible: bool,
    /// Horizontal offset in pixels (for positioning).
    pub offset_x: i32,
    /// Vertical offset in pixels (for positioning).
    pub offset_y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayerSource {
    /// An image loaded from disk.
    Image(PathBuf),
    /// A clone/reference to the same source as another layer.
    CloneOf(u32),
    /// A procedurally generated solid color.
    SolidColor { r: u8, g: u8, b: u8 },
    /// A procedurally generated gradient.
    Gradient { start: [u8; 3], end: [u8; 3], angle: f32 },
    /// A procedurally generated noise texture.
    Noise { seed: u32, scale: f32, monochromatic: bool },
}
```

### 4.2 Composition

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Composition {
    /// Ordered list of layers, bottom-to-top.
    pub layers: Vec<Layer>,
    /// Index of the currently active (selected) layer.
    pub active_layer: usize,
    /// Canvas dimensions (defaults to the first image layer's dimensions).
    pub width: u32,
    pub height: u32,
}
```

### 4.3 BlendMode (expanded)

```rust
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    ColorDodge,
    ColorBurn,
    Add,
    Subtract,
}

impl BlendMode {
    /// Blends a top pixel onto a bottom pixel.
    /// All values are 0.0–1.0 normalised.
    pub fn blend(self, bottom: [f32; 3], top: [f32; 3]) -> [f32; 3] {
        match self {
            Self::Normal    => top,
            Self::Multiply  => [bottom[0]*top[0], bottom[1]*top[1], bottom[2]*top[2]],
            Self::Screen    => {
                let f = |b: f32, t: f32| 1.0 - (1.0-b)*(1.0-t);
                [f(bottom[0],top[0]), f(bottom[1],top[1]), f(bottom[2],top[2])]
            },
            Self::Overlay   => {
                let f = |b: f32, t: f32| {
                    if b < 0.5 { 2.0*b*t } else { 1.0 - 2.0*(1.0-b)*(1.0-t) }
                };
                [f(bottom[0],top[0]), f(bottom[1],top[1]), f(bottom[2],top[2])]
            },
            // ... additional modes follow the same pattern
            _ => top,
        }
    }
}
```

### 4.4 AppState Changes

```rust
// New fields in AppState:
pub composition: Composition,
/// Per-layer proxy assets (downscaled to proxy resolution).
pub layer_proxies: Vec<Option<DynamicImage>>,
/// Per-layer rendered output (after pipeline application).
pub layer_renders: Vec<Option<DynamicImage>>,
/// The final composited output.
pub composited_preview: Option<DynamicImage>,
```

The existing `source_asset`, `proxy_asset`, `pipeline`, and `preview_buffer`
become properties of the **active layer** in the composition. Legacy
single-layer mode is preserved: a new image loaded without layers creates a
`Composition` with a single `Layer`.

---

## 5. TUI Layout

### 5.1 Layers Panel

When layers mode is active, a new **Layers Panel** appears between the Canvas
and the Effects Panel:

```
┌──────────────────────────────────────────────────────────────────┐
│ Status Bar                                                       │
├───────────────────────────┬──────────────┬───────────────────────┤
│                           │              │                       │
│  Canvas                   │  Layers (3)  │  Effects (active)     │
│  (composited preview)     │              │                       │
│                           │  ► Base      │  [✓] Contrast         │
│                           │    Glitch    │  [✓] Vignette         │
│                           │    Noise     │                       │
│                           │              │                       │
├───────────────────────────┴──────────────┴───────────────────────┤
│ Controls Hint                                                    │
└──────────────────────────────────────────────────────────────────┘
```

The Layers Panel is 14 columns wide and shows:

| Row Element | Description |
|-------------|-------------|
| `►` marker | Active layer indicator |
| Layer name | Truncated to 10 chars |
| Eye icon `👁` / `·` | Visible / hidden |
| Opacity percentage | e.g., `70%` |
| Blend mode abbreviation | e.g., `Scr`, `Mul`, `Ovr` |

### 5.2 Focus Cycle

`Tab` cycle extends: **Canvas → Layers → Effects → Animation** (if open).

---

## 6. Keyboard Shortcuts

### 6.1 Normal Mode (new bindings)

| Key | Action |
|-----|--------|
| `Ctrl+L` | Toggle Layers Panel visibility |

### 6.2 Layers Panel Focused

| Key | Action |
|-----|--------|
| `↑` / `k` | Select previous layer |
| `↓` / `j` | Select next layer |
| `Shift+K` / `Shift+J` | Move layer up/down in stack |
| `Enter` | Edit active layer properties (name, blend, opacity) |
| `n` | New layer (opens source picker: Image, Solid, Gradient, Noise) |
| `d` / `Delete` | Delete selected layer (with confirmation if >1 layer) |
| `Space` | Toggle layer visibility |
| `c` | Duplicate selected layer (clones pipeline + settings) |
| `m` | Cycle blend mode (Normal → Multiply → Screen → ...) |
| `[` / `]` | Decrease / increase opacity by 5% |
| `Esc` | Return focus to Effects panel |
| `Tab` | Cycle focus to next panel |

### 6.3 Layer Properties Dialog

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate fields |
| `Tab` | Next field |
| Text input | Edit name, numeric fields |
| `Enter` | Confirm |
| `Esc` | Cancel |

Fields:

1. **Name** — text input (max 20 chars)
2. **Blend Mode** — dropdown (12 modes)
3. **Opacity** — slider (0–100%)
4. **Offset X** — numeric (pixels)
5. **Offset Y** — numeric (pixels)

---

## 7. Compositing Engine

### 7.1 Worker Commands

```rust
// New WorkerCommand variants:
WorkerCommand::ProcessLayer {
    layer_idx: usize,
    image: DynamicImage,
    pipeline: Pipeline,
    response_tx: Sender<WorkerResponse>,
},
WorkerCommand::CompositeAll {
    layer_renders: Vec<(DynamicImage, BlendMode, f32, i32, i32)>,
    canvas_width: u32,
    canvas_height: u32,
    response_tx: Sender<WorkerResponse>,
},

// New WorkerResponse variants:
WorkerResponse::LayerReady {
    layer_idx: usize,
    image: DynamicImage,
},
WorkerResponse::CompositeReady(DynamicImage),
```

### 7.2 Compositing Algorithm

```
fn composite(layers: &[(DynamicImage, BlendMode, f32, i32, i32)],
             width: u32, height: u32) -> DynamicImage {
    let mut canvas = RgbaImage::new(width, height);

    for (layer_img, blend_mode, opacity, off_x, off_y) in layers {
        for (x, y, top_pixel) in layer_img.pixels() {
            let cx = x as i32 + off_x;
            let cy = y as i32 + off_y;
            if cx < 0 || cy < 0 || cx >= width || cy >= height { continue; }

            let bottom = canvas.get_pixel(cx as u32, cy as u32);
            let blended = blend_mode.blend(bottom.rgb_f32(), top_pixel.rgb_f32());
            let mixed = lerp(bottom.rgb_f32(), blended, opacity * (top_pixel.a / 255.0));
            canvas.put_pixel(cx as u32, cy as u32, mixed.to_rgba8());
        }
    }

    DynamicImage::ImageRgba8(canvas)
}
```

### 7.3 Dirty Layer Tracking

Each layer gets a `dirty: bool` flag. When a layer's pipeline or source changes,
only that layer is re-processed — the composition step re-blends all layers but
skips re-rendering clean layers. This keeps preview updates fast when editing a
single layer in a multi-layer composition.

---

## 8. Pipeline Persistence

Compositions are saved as a superset of the existing pipeline JSON:

```json
{
  "composition": {
    "width": 1920,
    "height": 1080,
    "layers": [
      {
        "id": 1,
        "name": "Base",
        "source": { "type": "Image", "path": "photo.png" },
        "pipeline": { "effects": [ ... ] },
        "blend_mode": "Normal",
        "opacity": 1.0,
        "visible": true,
        "offset_x": 0,
        "offset_y": 0
      },
      {
        "id": 2,
        "name": "Glitch overlay",
        "source": { "type": "CloneOf", "layer_id": 1 },
        "pipeline": { "effects": [ ... ] },
        "blend_mode": "Screen",
        "opacity": 0.6,
        "visible": true,
        "offset_x": 10,
        "offset_y": -5
      }
    ]
  }
}
```

Loading a plain pipeline JSON (no `composition` key) creates a single-layer
composition automatically, maintaining backward compatibility.

---

## 9. Export Behaviour

| Export Type | Behaviour |
|-------------|-----------|
| Still image (`e`) | Composites all visible layers at full source resolution, then exports |
| Animation | Each frame composites all layers (each layer's pipeline may have different keyframes) |
| Pipeline save (`Ctrl+S`) | Saves full composition with all layer definitions |

---

## 10. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| Layer source image not found | Show `"⚠ Layer 'X': source not found"` in status bar; render as checkerboard |
| Delete last layer | Prevented — at least one layer must exist |
| Layer source file changed on disk | No auto-reload; user must re-open |
| Circular `CloneOf` reference | Prevented at creation time; UI disallows cloning a clone |
| `CloneOf` references a deleted layer | Resolve at deletion time: convert `CloneOf(id)` to `Image(path)` using the deleted layer's source, or prompt user |
| Very large composition (10+ layers) | Status bar warns about memory; proxy resolution auto-reduces |

---

## 11. File Changes Summary

| File | Change |
|------|--------|
| `src/app/layers.rs` *(new)* | `Layer`, `LayerSource`, `Composition`, `BlendMode` structs |
| `src/app/state.rs` | Add `composition`, `layer_proxies`, `layer_renders` fields; refactor pipeline accessors to delegate to active layer |
| `src/app/handlers.rs` | Add Layers panel key handlers; extend `Tab` cycle |
| `src/app/dialogs.rs` | Add `LayerPropertiesDialog`, `NewLayerDialog` |
| `src/app/mod.rs` | Re-export `layers` module |
| `src/engine/composite.rs` | New `composite()` function with blend mode math |
| `src/engine/worker.rs` | Handle `ProcessLayer` and `CompositeAll` commands |
| `src/ui/layers_panel.rs` *(new)* | Layers panel widget rendering |
| `src/ui/mod.rs` | Register layers panel in layout |
| `src/ui/layout.rs` | Add Layers panel column to layout computation |
| `src/config/parser.rs` | Extend save/load for composition JSON |
| `README.md` | Document layers feature, shortcuts, blend modes |

---

## 12. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Data model** | `Layer`, `Composition`, `BlendMode` structs; refactor `AppState` to delegate to active layer | Compiling model, single-layer backward compat |
| **2 — Layers panel UI** | Panel widget, `Tab` cycle extension, layer selection | Visual panel (no compositing yet) |
| **3 — Layer management** | Add, delete, duplicate, reorder, visibility toggle | Full layer CRUD |
| **4 — Compositing engine** | Blend mode math, `CompositeAll` worker command, dirty tracking | Layers composite correctly |
| **5 — Generated sources** | SolidColor, Gradient, Noise layer sources | Creative layering without external files |
| **6 — Layer properties** | Properties dialog (name, blend, opacity, offset) | Fine-tuned layer control |
| **7 — Persistence** | Save/load composition JSON, backward compat | Reloadable compositions |
| **8 — UX polish** | Status bar indicators, performance warnings, documentation | Feature complete |

---

## 13. Open Questions

1. **Layer thumbnails:** Should the Layers panel show tiny Sixel thumbnails
   (6×4 chars) of each layer's output? This would be visually rich but
   increases rendering cost. Recommendation: defer to a polish phase.

2. **Adjustment layers:** Should there be "pass-through" layers that apply
   effects to all layers below them (like Photoshop adjustment layers)?
   Recommendation: yes, but defer to Phase 9 — the current model supports
   this via a `LayerSource::PassThrough` variant.

3. **Layer groups:** Should layers be nestable into groups for complex
   compositions? Recommendation: defer — flat layer stacks cover 90% of
   use cases.

4. **Maximum layer count:** Should there be a hard limit? Recommendation:
   soft limit of 16 layers with a warning; no hard limit.
