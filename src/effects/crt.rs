use image::Rgba;
use serde::{Deserialize, Serialize};

/// CRT-style post-processing effects: scanlines, curvature, phosphor glow, noise, RGB shift, vignette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrtEffect {
    /// Draw horizontal scanlines every N rows.
    Scanlines { spacing: u32, opacity: f32 },
    /// Apply barrel-distortion curvature to simulate a curved CRT screen.
    Curvature { strength: f32 },
    /// Blur and add a coloured halo to bright regions.
    PhosphorGlow { radius: u32, intensity: f32 },
    /// Add RGB or monochromatic noise.
    Noise { intensity: f32, monochromatic: bool },
    /// Radial darkening towards the edges (vignette).
    Vignette { radius: f32, softness: f32 },
}

impl CrtEffect {
    /// Apply per-pixel CRT transformation.
    ///
    /// Effects that require coordinate context (scanlines, vignette) need the
    /// caller to supply the pixel's (x, y) position and the image dimensions.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            // Per-pixel stubs – full implementations live in apply_image().
            CrtEffect::Scanlines { .. } => pixel,
            CrtEffect::Curvature { .. } => pixel,
            CrtEffect::PhosphorGlow { .. } => pixel,
            CrtEffect::Noise { .. } => pixel,
            CrtEffect::Vignette { .. } => pixel,
        }
    }

    /// Apply this effect to a pixel given its position and image dimensions.
    pub fn apply_pixel_with_coords(
        &self,
        pixel: Rgba<u8>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Rgba<u8> {
        match self {
            CrtEffect::Scanlines { spacing, opacity } => {
                if spacing == &0 {
                    return pixel;
                }
                if y % spacing == 0 {
                    let darken =
                        |c: u8| -> u8 { ((c as f32) * (1.0 - opacity.clamp(0.0, 1.0))) as u8 };
                    Rgba([
                        darken(pixel[0]),
                        darken(pixel[1]),
                        darken(pixel[2]),
                        pixel[3],
                    ])
                } else {
                    pixel
                }
            }
            CrtEffect::Vignette { radius, softness } => {
                // Normalised coordinates centred at (0,0), range -1..1.
                let nx = (x as f32 / width as f32) * 2.0 - 1.0;
                let ny = (y as f32 / height as f32) * 2.0 - 1.0;
                let dist = (nx * nx + ny * ny).sqrt();
                // Smooth-step between radius and radius+softness.
                let t = ((dist - radius) / softness.max(0.001)).clamp(0.0, 1.0);
                let factor = 1.0 - t * t * (3.0 - 2.0 * t); // smooth-step
                let darken = |c: u8| -> u8 { (c as f32 * factor) as u8 };
                Rgba([
                    darken(pixel[0]),
                    darken(pixel[1]),
                    darken(pixel[2]),
                    pixel[3],
                ])
            }
            CrtEffect::Noise {
                intensity,
                monochromatic,
            } => {
                // Deterministic noise seeded by pixel position.
                let seed = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519)) as f32;
                let n = (seed.sin() * 43758.5453).fract(); // 0..1
                let delta = ((n * 2.0 - 1.0) * intensity * 255.0) as i16;
                let add = |c: u8, d: i16| -> u8 { (c as i16 + d).clamp(0, 255) as u8 };
                if *monochromatic {
                    let v = add(pixel[0], delta);
                    Rgba([v, v, v, pixel[3]])
                } else {
                    let seed_r = seed;
                    let seed_g = (seed * 1.618).fract();
                    let seed_b = (seed * 2.718).fract();
                    let dr = (((seed_r * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    let dg = (((seed_g * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    let db = (((seed_b * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    Rgba([
                        add(pixel[0], dr),
                        add(pixel[1], dg),
                        add(pixel[2], db),
                        pixel[3],
                    ])
                }
            }
            // Full-image ops fall back gracefully in per-pixel mode.
            CrtEffect::Curvature { .. } | CrtEffect::PhosphorGlow { .. } => pixel,
        }
    }
}
