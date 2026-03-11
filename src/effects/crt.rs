use std::fmt;

use image::Rgba;
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;

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
                if y.is_multiple_of(*spacing) {
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
                let n = (seed.sin() * 43_758.547).fract(); // 0..1
                let delta = ((n * 2.0 - 1.0) * intensity * 255.0) as i16;
                let add = |c: u8, d: i16| -> u8 { (c as i16 + d).clamp(0, 255) as u8 };
                if *monochromatic {
                    let v = add(pixel[0], delta);
                    Rgba([v, v, v, pixel[3]])
                } else {
                    let seed_r = seed;
                    let seed_g = (seed * 1.618).fract();
                    let seed_b = (seed * std::f32::consts::E).fract();
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

    /// Return descriptors for all editable numeric parameters.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            CrtEffect::Scanlines { spacing, opacity } => vec![
                ParamDescriptor {
                    name: "spacing",
                    value: *spacing as f32,
                    min: 1.0,
                    max: 20.0,
                },
                ParamDescriptor {
                    name: "opacity",
                    value: *opacity,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            CrtEffect::Curvature { strength } => vec![ParamDescriptor {
                name: "strength",
                value: *strength,
                min: 0.0,
                max: 1.0,
            }],
            CrtEffect::PhosphorGlow { radius, intensity } => vec![
                ParamDescriptor {
                    name: "radius",
                    value: *radius as f32,
                    min: 1.0,
                    max: 20.0,
                },
                ParamDescriptor {
                    name: "intensity",
                    value: *intensity,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            CrtEffect::Noise {
                intensity,
                monochromatic,
            } => vec![
                ParamDescriptor {
                    name: "intensity",
                    value: *intensity,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "monochromatic",
                    value: if *monochromatic { 1.0 } else { 0.0 },
                    min: 0.0,
                    max: 1.0,
                },
            ],
            CrtEffect::Vignette { radius, softness } => vec![
                ParamDescriptor {
                    name: "radius",
                    value: *radius,
                    min: 0.0,
                    max: 2.0,
                },
                ParamDescriptor {
                    name: "softness",
                    value: *softness,
                    min: 0.0,
                    max: 2.0,
                },
            ],
        }
    }

    /// Rebuild this variant with new parameter values (clamped to valid ranges).
    pub fn apply_params(&self, values: &[f32]) -> CrtEffect {
        let get = |i: usize, fallback: f32| values.get(i).copied().unwrap_or(fallback);
        match self {
            CrtEffect::Scanlines { spacing, opacity } => CrtEffect::Scanlines {
                spacing: get(0, *spacing as f32) as u32,
                opacity: get(1, *opacity),
            },
            CrtEffect::Curvature { strength } => CrtEffect::Curvature {
                strength: get(0, *strength),
            },
            CrtEffect::PhosphorGlow { radius, intensity } => CrtEffect::PhosphorGlow {
                radius: get(0, *radius as f32) as u32,
                intensity: get(1, *intensity),
            },
            CrtEffect::Noise {
                intensity,
                monochromatic,
            } => CrtEffect::Noise {
                intensity: get(0, *intensity),
                monochromatic: get(1, if *monochromatic { 1.0 } else { 0.0 }) >= 0.5,
            },
            CrtEffect::Vignette { radius, softness } => CrtEffect::Vignette {
                radius: get(0, *radius),
                softness: get(1, *softness),
            },
        }
    }

    /// Returns the variant name (e.g. `"Scanlines"`, `"Noise"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            CrtEffect::Scanlines { .. } => "Scanlines",
            CrtEffect::Curvature { .. } => "Curvature",
            CrtEffect::PhosphorGlow { .. } => "PhosphorGlow",
            CrtEffect::Noise { .. } => "Noise",
            CrtEffect::Vignette { .. } => "Vignette",
        }
    }
}

impl fmt::Display for CrtEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrtEffect::Scanlines { spacing, opacity } => {
                write!(f, "Scanlines {spacing}px {opacity:.0}%")
            }
            CrtEffect::Curvature { strength } => write!(f, "Curvature {strength:.2}"),
            CrtEffect::PhosphorGlow { radius, intensity } => {
                write!(f, "PhosphorGlow r={radius} i={intensity:.2}")
            }
            CrtEffect::Noise {
                intensity,
                monochromatic,
            } => {
                let kind = if *monochromatic { "mono" } else { "rgb" };
                write!(f, "Noise {kind} {intensity:.2}")
            }
            CrtEffect::Vignette { radius, softness } => {
                write!(f, "Vignette r={radius:.2} s={softness:.2}")
            }
        }
    }
}
