use crate::effects::{
    Effect, EnabledEffect, Pipeline, color, color::ColorEffect, composite::CompositeEffect,
    crt::CrtEffect, glitch::GlitchEffect,
};

/// All effects available to add, with display names.
pub type EffectEntry = (&'static str, fn() -> Effect);

pub const AVAILABLE_EFFECTS: &[EffectEntry] = &[
    // ── Color ────────────────────────────────────────────────────────────
    ("Invert", || Effect::Color(ColorEffect::Invert)),
    ("Gradient Map", || {
        Effect::Color(ColorEffect::GradientMap {
            preset_idx: 0,
            stops: color::GRADIENT_PRESETS[0].1.to_vec(),
        })
    }),
    ("HueShift +30°", || {
        Effect::Color(ColorEffect::HueShift { degrees: 30.0 })
    }),
    ("Contrast ×1.5", || {
        Effect::Color(ColorEffect::Contrast { factor: 1.5 })
    }),
    ("Saturation ×1.5", || {
        Effect::Color(ColorEffect::Saturation { factor: 1.5 })
    }),
    ("Desaturate", || {
        Effect::Color(ColorEffect::Saturation { factor: 0.0 })
    }),
    ("Quantize (4 levels)", || {
        Effect::Color(ColorEffect::ColorQuantization { levels: 4 })
    }),
    // ── Glitch ───────────────────────────────────────────────────────────
    ("Pixelate (8px)", || {
        Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })
    }),
    ("Row Jitter", || {
        Effect::Glitch(GlitchEffect::RowJitter {
            magnitude: 0.05,
            seed: 0,
        })
    }),
    ("Block Shift", || {
        Effect::Glitch(GlitchEffect::BlockShift {
            shift_x: 10,
            shift_y: 0,
        })
    }),
    ("Pixel Sort", || {
        Effect::Glitch(GlitchEffect::PixelSort {
            threshold: 0.5,
            reverse: false,
        })
    }),
    ("Fractal Julia", || {
        Effect::Glitch(GlitchEffect::FractalJulia {
            scale: 2.5,
            cx: -0.7,
            cy: 0.27015,
            max_iter: 80,
            blend: 0.5,
        })
    }),
    ("Delaunay Triangulation", || {
        Effect::Glitch(GlitchEffect::DelaunayTriangulation {
            num_points: 200,
            seed: 42,
        })
    }),
    ("Ghost Displace", || {
        Effect::Glitch(GlitchEffect::GhostDisplace {
            copies: 5,
            offset_x: 8.0,
            offset_y: 4.0,
            hue_sweep: 60.0,
            opacity: 0.4,
        })
    }),
    // ── CRT ──────────────────────────────────────────────────────────────
    ("Scanlines", || {
        Effect::Crt(CrtEffect::Scanlines {
            spacing: 2,
            opacity: 0.5,
            color_r: 0,
            color_g: 0,
            color_b: 0,
        })
    }),
    ("Noise (RGB)", || {
        Effect::Crt(CrtEffect::Noise {
            intensity: 0.1,
            monochromatic: false,
            seed: 0,
        })
    }),
    ("Vignette", || {
        Effect::Crt(CrtEffect::Vignette {
            radius: 0.7,
            softness: 0.3,
        })
    }),
    // ── Composite ────────────────────────────────────────────────────────
    ("Crop 50%", || {
        Effect::Composite(CompositeEffect::CropRect {
            x: 50,
            y: 50,
            width: 200,
            height: 200,
        })
    }),
];

/// Randomize the numeric parameters of every effect in the pipeline.
///
/// Populates the pipeline with 2–5 random effects so randomization is
/// visible even when starting from an empty pipeline, then tweaks parameters
/// for each effect using a cheap LCG PRNG seeded from the system clock.
pub fn randomize_pipeline(pipeline: &mut Pipeline) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    let seed = hasher.finish();

    // LCG: cheap deterministic random sequence from seed.
    let mut rng = seed;
    let mut next = move || -> f32 {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((rng >> 33) as f32) / (u32::MAX as f32)
    };

    // Populate the pipeline with 2-5 random effects.
    let count = 2 + (next() * 4.0) as usize;
    pipeline.effects.clear();
    for _ in 0..count {
        let idx = (next() * AVAILABLE_EFFECTS.len() as f32) as usize % AVAILABLE_EFFECTS.len();
        pipeline
            .effects
            .push(EnabledEffect::new(AVAILABLE_EFFECTS[idx].1()));
    }

    for ee in &mut pipeline.effects {
        match &mut ee.effect {
            Effect::Color(e) => match e {
                ColorEffect::HueShift { degrees } => *degrees = next() * 360.0,
                ColorEffect::Contrast { factor } => *factor = 0.5 + next() * 2.5,
                ColorEffect::Saturation { factor } => *factor = next() * 2.0,
                ColorEffect::ColorQuantization { levels } => *levels = 2 + (next() * 6.0) as u8,
                ColorEffect::Invert => {}
                ColorEffect::GradientMap { preset_idx, stops } => {
                    *preset_idx = (next() * color::GRADIENT_PRESETS.len() as f32) as usize
                        % color::GRADIENT_PRESETS.len();
                    *stops = color::GRADIENT_PRESETS[*preset_idx].1.to_vec();
                }
            },
            Effect::Glitch(e) => match e {
                GlitchEffect::Pixelate { block_size } => *block_size = 2 + (next() * 20.0) as u32,
                GlitchEffect::RowJitter { magnitude, seed } => {
                    *magnitude = next() * 0.2;
                    *seed = (next() * 9999.0) as u32;
                }
                GlitchEffect::BlockShift { shift_x, shift_y } => {
                    *shift_x = ((next() - 0.5) * 40.0) as i32;
                    *shift_y = ((next() - 0.5) * 40.0) as i32;
                }
                GlitchEffect::PixelSort { threshold, reverse } => {
                    *threshold = 0.2 + next() * 0.6;
                    *reverse = next() >= 0.5;
                }
                GlitchEffect::FractalJulia {
                    scale,
                    cx,
                    cy,
                    max_iter,
                    blend,
                } => {
                    *scale = 0.5 + next() * 4.0;
                    *cx = (next() - 0.5) * 3.0;
                    *cy = (next() - 0.5) * 3.0;
                    *max_iter = 20 + (next() * 180.0) as u32;
                    *blend = 0.2 + next() * 0.6;
                }
                GlitchEffect::DelaunayTriangulation { num_points, seed } => {
                    *num_points = 50 + (next() * 500.0) as u32;
                    *seed = (next() * 9999.0) as u32;
                }
                GlitchEffect::GhostDisplace {
                    copies,
                    offset_x,
                    offset_y,
                    hue_sweep,
                    opacity,
                } => {
                    *copies = 2 + (next() * 8.0) as u32;
                    *offset_x = (next() - 0.5) * 40.0;
                    *offset_y = (next() - 0.5) * 40.0;
                    *hue_sweep = next() * 360.0;
                    *opacity = 0.2 + next() * 0.6;
                }
            },
            Effect::Crt(e) => match e {
                CrtEffect::Scanlines {
                    spacing,
                    opacity,
                    color_r,
                    color_g,
                    color_b,
                } => {
                    *spacing = 2 + (next() * 4.0) as u32;
                    *opacity = 0.3 + next() * 0.7;
                    *color_r = (next() * 64.0) as u8;
                    *color_g = (next() * 64.0) as u8;
                    *color_b = (next() * 64.0) as u8;
                }
                CrtEffect::Noise {
                    intensity, seed, ..
                } => {
                    *intensity = next() * 0.3;
                    *seed = (next() * 9999.0) as u32;
                }
                CrtEffect::Vignette { radius, softness } => {
                    *radius = 0.3 + next() * 0.5;
                    *softness = 0.1 + next() * 0.5;
                }
                CrtEffect::Curvature { strength } => *strength = next(),
                CrtEffect::PhosphorGlow { radius, intensity } => {
                    *radius = 1 + (next() * 5.0) as u32;
                    *intensity = next();
                }
            },
            Effect::Composite(_) => {}
        }
    }
}

/// Format a float parameter value for display in the edit buffer.
///
/// Integers (where `fract() == 0`) are displayed without a decimal point
/// (e.g. `8` instead of `8.0`), while fractional values use Rust's default
/// shortest-round-trip representation (e.g. `0.05`, `1.5`).
pub fn format_param_value(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}
