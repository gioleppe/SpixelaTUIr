# Procedural Texture Generation — Design Specification

## 1. Overview

This document specifies the design for a **procedural texture generation**
system in Spix. Rather than loading an image from disk, users can generate
images algorithmically — noise fields, fractal patterns, geometric tilings,
and color gradients — and feed them directly into the effect pipeline. This
enables fully self-contained creative workflows: generate a texture, glitch it,
animate it, and export — all without leaving Spix.

Procedural textures also serve as powerful blend sources for the Layer &
Compositing System (see `layer_system_specs.md`) and as mask generators for
the Masking System (see `masking_system_specs.md`).

---

## 2. Motivation

| Limitation | Desired Outcome |
|------------|-----------------|
| Spix requires an external image file to do anything | Users can create from scratch inside Spix |
| No way to generate test patterns for effect development | Built-in test patterns (gradient, checkerboard, noise) |
| FractalJulia is the only generative effect | More generative algorithms (Perlin noise, Voronoi, Mandelbrot) |
| Blend/layer sources are limited to loaded files | Procedural textures as infinite blend material |

---

## 3. Texture Types

### 3.1 Noise-Based

| Texture | Description | Parameters |
|---------|-------------|------------|
| **Perlin Noise** | Classic smooth gradient noise | `scale`, `octaves`, `persistence`, `lacunarity`, `seed` |
| **Simplex Noise** | Improved Perlin (fewer artifacts) | `scale`, `octaves`, `persistence`, `seed` |
| **Worley / Voronoi** | Cell-based distance field | `num_cells`, `distance_metric` (Euclidean, Manhattan, Chebyshev), `seed` |
| **Value Noise** | Random grid with interpolation | `scale`, `seed` |
| **White Noise** | Uniform random per pixel | `seed` |
| **FBM (Fractal Brownian Motion)** | Multi-octave noise composite | `noise_type`, `octaves`, `gain`, `frequency`, `seed` |

### 3.2 Geometric Patterns

| Texture | Description | Parameters |
|---------|-------------|------------|
| **Checkerboard** | Alternating squares | `cell_size`, `color_a`, `color_b` |
| **Stripes** | Horizontal, vertical, or diagonal bands | `width`, `angle`, `color_a`, `color_b` |
| **Grid** | Line grid overlay | `spacing`, `line_width`, `color`, `background` |
| **Concentric Circles** | Rings from center | `spacing`, `line_width`, `color`, `background` |
| **Dots** | Regular dot pattern | `spacing`, `radius`, `color`, `background` |
| **Hexagonal Tiling** | Honeycomb pattern | `cell_size`, `line_width`, `color_a`, `color_b` |

### 3.3 Fractal

| Texture | Description | Parameters |
|---------|-------------|------------|
| **Mandelbrot Set** | Classic fractal (zoom, pan, iterations) | `center_x`, `center_y`, `zoom`, `max_iter`, `color_map` |
| **Julia Set** | Parameterised fractal (extends existing `FractalJulia` effect) | `cx`, `cy`, `zoom`, `max_iter`, `color_map` |
| **Burning Ship** | Variant fractal | `center_x`, `center_y`, `zoom`, `max_iter` |
| **Sierpinski Triangle** | Self-similar triangle | `depth`, `color`, `background` |

### 3.4 Gradients

| Texture | Description | Parameters |
|---------|-------------|------------|
| **Linear Gradient** | Two-color gradient at any angle | `color_a`, `color_b`, `angle` |
| **Radial Gradient** | Circular gradient from center | `color_center`, `color_edge`, `center_x`, `center_y` |
| **Conic Gradient** | Angular sweep gradient | `colors: Vec<[u8;3]>`, `center_x`, `center_y` |
| **Multi-Stop Gradient** | N-stop linear gradient | `stops: Vec<(f32, [u8;3])>`, `angle` |

---

## 4. Data Structures

### 4.1 ProceduralTexture Enum

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProceduralTexture {
    // Noise
    PerlinNoise {
        scale: f32,
        octaves: u32,
        persistence: f32,
        lacunarity: f32,
        seed: u32,
    },
    SimplexNoise {
        scale: f32,
        octaves: u32,
        persistence: f32,
        seed: u32,
    },
    Voronoi {
        num_cells: u32,
        distance_metric: DistanceMetric,
        seed: u32,
    },
    WhiteNoise { seed: u32 },
    Fbm {
        noise_type: NoiseType,
        octaves: u32,
        gain: f32,
        frequency: f32,
        seed: u32,
    },

    // Geometric
    Checkerboard {
        cell_size: u32,
        color_a: [u8; 3],
        color_b: [u8; 3],
    },
    Stripes {
        width: u32,
        angle: f32,
        color_a: [u8; 3],
        color_b: [u8; 3],
    },
    Grid {
        spacing: u32,
        line_width: u32,
        color: [u8; 3],
        background: [u8; 3],
    },
    Dots {
        spacing: u32,
        radius: u32,
        color: [u8; 3],
        background: [u8; 3],
    },
    HexTiling {
        cell_size: u32,
        line_width: u32,
        color_a: [u8; 3],
        color_b: [u8; 3],
    },

    // Fractal
    Mandelbrot {
        center_x: f64,
        center_y: f64,
        zoom: f64,
        max_iter: u32,
        color_map: ColorMapPreset,
    },
    BurningShip {
        center_x: f64,
        center_y: f64,
        zoom: f64,
        max_iter: u32,
    },

    // Gradients
    LinearGradient {
        color_a: [u8; 3],
        color_b: [u8; 3],
        angle: f32,
    },
    RadialGradient {
        color_center: [u8; 3],
        color_edge: [u8; 3],
        center_x: f32,
        center_y: f32,
    },
    ConicGradient {
        colors: Vec<[u8; 3]>,
        center_x: f32,
        center_y: f32,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DistanceMetric {
    Euclidean,
    Manhattan,
    Chebyshev,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NoiseType {
    Perlin,
    Simplex,
    Value,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ColorMapPreset {
    Grayscale,
    Fire,
    Ice,
    Rainbow,
    Monochrome,
}
```

### 4.2 AppState Changes

```rust
// In AppState:
/// The currently active procedural texture generator (if source is procedural).
pub procedural_source: Option<ProceduralTexture>,
```

When `procedural_source` is `Some`, the `source_asset` is regenerated from
the texture whenever its parameters change, instead of being loaded from disk.

---

## 5. Generation Engine

### 5.1 Generator Trait

```rust
pub trait TextureGenerator {
    fn generate(&self, width: u32, height: u32) -> DynamicImage;
    fn param_descriptors(&self) -> Vec<ParamDescriptor>;
    fn apply_params(&mut self, idx: usize, value: f32);
}
```

Each `ProceduralTexture` variant implements this trait.

### 5.2 Noise Implementation

For Perlin/Simplex/Value noise, implement from scratch using standard
algorithms to avoid adding a dependency:

```rust
fn perlin_2d(x: f32, y: f32, seed: u32) -> f32 {
    // Hash-based gradient noise
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - x.floor();
    let yf = y - y.floor();

    let u = fade(xf);
    let v = fade(yf);

    let g00 = grad(hash(xi, yi, seed), xf, yf);
    let g10 = grad(hash(xi + 1, yi, seed), xf - 1.0, yf);
    let g01 = grad(hash(xi, yi + 1, seed), xf, yf - 1.0);
    let g11 = grad(hash(xi + 1, yi + 1, seed), xf - 1.0, yf - 1.0);

    let x0 = lerp(g00, g10, u);
    let x1 = lerp(g01, g11, u);
    lerp(x0, x1, v)
}

fn fbm(x: f32, y: f32, octaves: u32, persistence: f32,
       lacunarity: f32, seed: u32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for i in 0..octaves {
        total += perlin_2d(x * frequency, y * frequency, seed + i) * amplitude;
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value  // normalize to [-1, 1]
}
```

### 5.3 Parallelisation

Texture generation is embarrassingly parallel — each pixel is independent.
Use `rayon` for all generators:

```rust
fn generate_perlin(params: &PerlinParams, width: u32, height: u32) -> DynamicImage {
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    pixels.par_chunks_exact_mut(4).enumerate().for_each(|(i, chunk)| {
        let x = (i as u32 % width) as f32 / width as f32 * params.scale;
        let y = (i as u32 / width) as f32 / height as f32 * params.scale;

        let value = fbm(x, y, params.octaves, params.persistence,
                        params.lacunarity, params.seed);
        let byte = ((value * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;

        chunk[0] = byte; // R
        chunk[1] = byte; // G
        chunk[2] = byte; // B
        chunk[3] = 255;  // A
    });

    DynamicImage::ImageRgba8(
        image::RgbaImage::from_raw(width, height, pixels).unwrap()
    )
}
```

---

## 6. TUI Integration

### 6.1 New Image Source Menu

Pressing `o` (Open) presents an extended menu:

```
┌──────────────────────────────────────────┐
│ New Image Source                         │
│                                          │
│ ► Open file from disk...                 │
│   Generate: Perlin Noise                 │
│   Generate: Voronoi                      │
│   Generate: Checkerboard                 │
│   Generate: Mandelbrot                   │
│   Generate: Linear Gradient              │
│   Generate: Radial Gradient              │
│   ... (scrollable)                       │
│                                          │
│ Enter: Select  │  Esc: Cancel            │
└──────────────────────────────────────────┘
```

Selecting a procedural source opens a parameters dialog:

```
┌──────────────────────────────────────────┐
│ Perlin Noise Parameters                  │
│                                          │
│ Width:       1024                         │
│ Height:      1024                         │
│ Scale:       [====●=====] 4.0            │
│ Octaves:     [===●======] 6              │
│ Persistence: [==●=======] 0.5            │
│ Lacunarity:  [===●======] 2.0            │
│ Seed:        [random]    42              │
│                                          │
│ Enter: Generate  │  r: Random  │  Esc    │
└──────────────────────────────────────────┘
```

### 6.2 Live Parameter Editing

Once a procedural texture is active, the user can re-open the parameters
dialog to tweak values and regenerate the texture. The pipeline is then
re-applied to the new texture automatically.

### 6.3 Status Bar

```
Ready | Perlin Noise [512px]                           1024×1024
```

The path is replaced by the texture type name when the source is procedural.

---

## 7. Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `o` | Open image source menu (extended with procedural options) |
| `Ctrl+G` | Re-open procedural texture parameters (when procedural source is active) |

### Texture Parameters Dialog

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate fields |
| `←` / `→` | Decrease / increase slider value |
| `Shift+←` / `Shift+→` | Coarse adjustment (10× step) |
| `r` | Randomise all parameters |
| `Enter` | Generate texture |
| `Esc` | Cancel |

---

## 8. Pipeline Persistence

Procedural textures are saved in the pipeline JSON:

```json
{
  "source": {
    "type": "ProceduralTexture",
    "texture": {
      "type": "PerlinNoise",
      "scale": 4.0,
      "octaves": 6,
      "persistence": 0.5,
      "lacunarity": 2.0,
      "seed": 42
    },
    "width": 1024,
    "height": 1024
  },
  "effects": [ ... ]
}
```

Loading a pipeline with a procedural source regenerates the texture from
the saved parameters — the texture itself is not stored in the file.

---

## 9. Integration with Other Features

| Feature | Integration |
|---------|-------------|
| **Animation** | Sweep a noise `seed` or `scale` parameter across frames for animated textures |
| **Layers** | Use procedural textures as layer sources (see `layer_system_specs.md`, `LayerSource::Noise`) |
| **Masking** | Generate masks from procedural textures (e.g., Voronoi cell boundaries as mask edges) |
| **Export** | Export the generated texture (before effects) as a standalone PNG |
| **Batch** | Combine batch mode with procedural sources for generating texture atlases |

---

## 10. Error Handling

| Situation | Behaviour |
|-----------|-----------|
| Invalid parameter (e.g., scale = 0) | Clamp to minimum valid value; show warning |
| Generation takes too long (>5 s) | Show progress indicator; allow cancellation with `Esc` |
| Out-of-memory for large textures | Limit maximum resolution to 8192×8192; show warning if exceeded |

---

## 11. File Changes Summary

| File | Change |
|------|--------|
| `src/effects/procedural.rs` *(new)* | `ProceduralTexture` enum, all generator implementations, `TextureGenerator` trait |
| `src/app/state.rs` | Add `procedural_source` field; texture regeneration logic |
| `src/app/handlers.rs` | Extend `o` key handler; `Ctrl+G` for re-editing; texture parameter dialog handlers |
| `src/app/dialogs.rs` | `TextureSourcePickerDialog`, `TextureParametersDialog` |
| `src/app/pipeline_utils.rs` | Texture presets for randomisation |
| `src/engine/worker.rs` | Handle texture generation (could run on worker thread for large textures) |
| `src/ui/widgets.rs` | Status bar: texture name instead of file path |
| `src/config/parser.rs` | Serialize/deserialize procedural sources |
| `README.md` | Document procedural textures, keyboard shortcuts |

---

## 12. Implementation Phases

| Phase | Scope | Deliverable |
|-------|-------|-------------|
| **1 — Generator framework** | `TextureGenerator` trait, `ProceduralTexture` enum, integration with `source_asset` | Framework compiles |
| **2 — Noise generators** | Perlin, Simplex, FBM, White Noise implementations | Noise textures generate correctly |
| **3 — Geometric patterns** | Checkerboard, Stripes, Grid, Dots, HexTiling | Pattern textures available |
| **4 — Fractals** | Mandelbrot, Burning Ship (extend existing Julia) | Fractal textures available |
| **5 — Gradients** | Linear, Radial, Conic, Multi-stop | Gradient textures available |
| **6 — Source picker UI** | Extended `o` menu, texture parameters dialog | User can generate textures |
| **7 — Live editing** | `Ctrl+G` to re-edit params; auto-regeneration | Interactive texture tweaking |
| **8 — Persistence** | Save/load procedural sources in pipeline JSON | Reloadable procedural pipelines |
| **9 — Voronoi** | Voronoi diagram generation with distance metrics | Cell-based textures |

---

## 13. Open Questions

1. **Dependency vs hand-rolled:** Should noise functions use the `noise` crate
   (well-tested, optimised) or be implemented from scratch (zero dependencies)?
   Recommendation: hand-roll for noise (small, well-understood algorithms) but
   consider the `noise` crate if performance or correctness issues arise.

2. **3D noise for animation:** Should noise generators support a third
   dimension (time) for seamless animated noise? Recommendation: yes — add an
   optional `z` / `time` parameter that advances per animation frame.

3. **Tile-ability:** Should textures be seamlessly tileable by default?
   Recommendation: offer a `tileable: bool` option that uses periodic noise
   functions.

4. **Colour mapping for noise:** Noise generators produce grayscale by default.
   Should there be a built-in color mapping option (e.g., fire, ice, rainbow)?
   Recommendation: yes — reuse the `ColorMapPreset` enum from the Mandelbrot
   generator for all noise types.

5. **Resolution independence:** Should procedural textures be regenerated when
   the proxy resolution changes (since they're computed, not loaded)?
   Recommendation: yes — regenerate at proxy resolution for preview, at full
   specified resolution for export.
