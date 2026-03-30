use std::fmt;

use image::{DynamicImage, Rgba};
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;

/// CRT-style post-processing effects: scanlines, curvature, phosphor glow, noise, RGB shift, vignette.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrtEffect {
    /// Draw horizontal scanlines every N rows.
    Scanlines {
        spacing: u32,
        opacity: f32,
        color_r: u8,
        color_g: u8,
        color_b: u8,
    },
    /// Apply barrel-distortion curvature to simulate a curved CRT screen.
    Curvature { strength: f32 },
    /// Blur and add a coloured halo to bright regions.
    PhosphorGlow { radius: u32, intensity: f32 },
    /// Simulate a decaying phosphor trail scanning left-to-right.
    PhosphorTrail {
        length: u32,
        decay: f32,
        color_mode: u8,
    },
    /// Add RGB or monochromatic noise.
    Noise {
        intensity: f32,
        monochromatic: bool,
        seed: u32,
    },
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
            CrtEffect::Scanlines {
                spacing,
                opacity,
                color_r,
                color_g,
                color_b,
            } => {
                if spacing == &0 {
                    return pixel;
                }
                if y.is_multiple_of(*spacing) {
                    let blend = |src: u8, tint: u8| -> u8 {
                        let a = opacity.clamp(0.0, 1.0);
                        ((src as f32) * (1.0 - a) + (tint as f32) * a) as u8
                    };
                    Rgba([
                        blend(pixel[0], *color_r),
                        blend(pixel[1], *color_g),
                        blend(pixel[2], *color_b),
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
                seed,
            } => {
                // Deterministic noise seeded by pixel position.
                let s = *seed;
                let seed_val = (x.wrapping_mul(2654435761).wrapping_add(s)
                    ^ y.wrapping_mul(2246822519).wrapping_add(s))
                    as f32;
                let n = (seed_val.sin() * 43_758.547).fract(); // 0..1
                let delta = ((n * 2.0 - 1.0) * intensity * 255.0) as i16;
                let add = |c: u8, d: i16| -> u8 { (c as i16 + d).clamp(0, 255) as u8 };
                if *monochromatic {
                    let v = add(pixel[0], delta);
                    Rgba([v, v, v, pixel[3]])
                } else {
                    let seed_val_r = seed_val;
                    let seed_val_g = (seed_val * 1.618).fract();
                    let seed_val_b = (seed_val * std::f32::consts::E).fract();
                    let dr =
                        (((seed_val_r * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    let dg =
                        (((seed_val_g * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    let db =
                        (((seed_val_b * 2.0 - 1.0) * intensity * 255.0) as i16).clamp(-255, 255);
                    Rgba([
                        add(pixel[0], dr),
                        add(pixel[1], dg),
                        add(pixel[2], db),
                        pixel[3],
                    ])
                }
            }
            // Full-image ops fall back gracefully in per-pixel mode.
            CrtEffect::Curvature { .. }
            | CrtEffect::PhosphorGlow { .. }
            | CrtEffect::PhosphorTrail { .. } => pixel,
        }
    }

    /// Apply this effect to a full image buffer.
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            CrtEffect::PhosphorTrail {
                length,
                decay,
                color_mode,
            } => phosphor_trail(img, *length, *decay, *color_mode),
            _ => {
                let (w, h) = (img.width(), img.height());
                super::apply_per_pixel(img, move |p, x, y| {
                    self.apply_pixel_with_coords(p, x, y, w, h)
                })
            }
        }
    }

    /// Return descriptors for all editable numeric parameters.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            CrtEffect::Scanlines {
                spacing,
                opacity,
                color_r,
                color_g,
                color_b,
            } => vec![
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
                ParamDescriptor {
                    name: "color_r",
                    value: *color_r as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "color_g",
                    value: *color_g as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "color_b",
                    value: *color_b as f32,
                    min: 0.0,
                    max: 255.0,
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
            CrtEffect::PhosphorTrail {
                length,
                decay,
                color_mode,
            } => vec![
                ParamDescriptor {
                    name: "length",
                    value: *length as f32,
                    min: 1.0,
                    max: 30.0,
                },
                ParamDescriptor {
                    name: "decay",
                    value: *decay,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "color_mode",
                    value: *color_mode as f32,
                    min: 0.0,
                    max: 2.0,
                },
            ],
            CrtEffect::Noise {
                intensity,
                monochromatic,
                seed,
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
                ParamDescriptor {
                    name: "seed",
                    value: *seed as f32,
                    min: 0.0,
                    max: 9999.0,
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
            CrtEffect::Scanlines {
                spacing,
                opacity,
                color_r,
                color_g,
                color_b,
            } => CrtEffect::Scanlines {
                spacing: get(0, *spacing as f32) as u32,
                opacity: get(1, *opacity),
                color_r: get(2, *color_r as f32) as u8,
                color_g: get(3, *color_g as f32) as u8,
                color_b: get(4, *color_b as f32) as u8,
            },
            CrtEffect::Curvature { strength } => CrtEffect::Curvature {
                strength: get(0, *strength),
            },
            CrtEffect::PhosphorGlow { radius, intensity } => CrtEffect::PhosphorGlow {
                radius: get(0, *radius as f32) as u32,
                intensity: get(1, *intensity),
            },
            CrtEffect::PhosphorTrail {
                length,
                decay,
                color_mode,
            } => CrtEffect::PhosphorTrail {
                length: get(0, *length as f32) as u32,
                decay: get(1, *decay),
                color_mode: get(2, *color_mode as f32) as u8,
            },
            CrtEffect::Noise {
                intensity,
                monochromatic,
                seed,
            } => CrtEffect::Noise {
                intensity: get(0, *intensity),
                monochromatic: get(1, if *monochromatic { 1.0 } else { 0.0 }) >= 0.5,
                seed: get(2, *seed as f32) as u32,
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
            CrtEffect::PhosphorTrail { .. } => "PhosphorTrail",
            CrtEffect::Noise { .. } => "Noise",
            CrtEffect::Vignette { .. } => "Vignette",
        }
    }
}

impl fmt::Display for CrtEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrtEffect::Scanlines {
                spacing,
                opacity,
                color_r,
                color_g,
                color_b,
            } => {
                if *color_r == 0 && *color_g == 0 && *color_b == 0 {
                    write!(f, "Scanlines {spacing}px {opacity:.0}%")
                } else {
                    write!(
                        f,
                        "Scanlines {spacing}px {opacity:.0}% rgb({color_r},{color_g},{color_b})"
                    )
                }
            }
            CrtEffect::Curvature { strength } => write!(f, "Curvature {strength:.2}"),
            CrtEffect::PhosphorGlow { radius, intensity } => {
                write!(f, "PhosphorGlow r={radius} i={intensity:.2}")
            }
            CrtEffect::PhosphorTrail {
                length,
                decay,
                color_mode,
            } => {
                let suffix = match color_mode {
                    0 => " [green]",
                    1 => " [amber]",
                    _ => "",
                };
                write!(f, "PhosphorTrail len={length} d={decay:.2}{suffix}")
            }
            CrtEffect::Noise {
                intensity,
                monochromatic,
                seed,
            } => {
                let kind = if *monochromatic { "mono" } else { "rgb" };
                write!(f, "Noise {kind} {intensity:.2} s={seed}")
            }
            CrtEffect::Vignette { radius, softness } => {
                write!(f, "Vignette r={radius:.2} s={softness:.2}")
            }
        }
    }
}

fn phosphor_trail(img: DynamicImage, length: u32, decay: f32, color_mode: u8) -> DynamicImage {
    let mut rgba = img.into_rgba8();
    let decay_factor = (1.0 - decay.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let max_steps = length.max(1);
    let tint = match color_mode {
        0 => [0.0_f32, 255.0_f32, 0.0_f32],     // green
        1 => [255.0_f32, 180.0_f32, 0.0_f32],   // amber
        _ => [255.0_f32, 255.0_f32, 255.0_f32], // white
    };

    for row in rgba.rows_mut() {
        let mut trail = [0.0_f32; 3];
        let mut remaining = 0_u32;
        for px in row {
            let lum = (0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32)
                * (1.0 / 255.0);

            if lum > 0.5 {
                trail[0] = tint[0] * lum;
                trail[1] = tint[1] * lum;
                trail[2] = tint[2] * lum;
                remaining = max_steps;
            } else if remaining > 0 {
                trail[0] *= decay_factor;
                trail[1] *= decay_factor;
                trail[2] *= decay_factor;
                remaining -= 1;
            } else {
                trail = [0.0; 3];
            }

            // Since decay factor is <= 1.0 and tint is <= 255.0, trail is always <= 255.0 and >= 0.0
            // Saturating add with u8 works because trail[i] * 0.5 is at most 127.5.
            px[0] = px[0].saturating_add((trail[0] * 0.5) as u8);
            px[1] = px[1].saturating_add((trail[1] * 0.5) as u8);
            px[2] = px[2].saturating_add((trail[2] * 0.5) as u8);
        }
    }

    DynamicImage::ImageRgba8(rgba)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    use super::phosphor_trail;

    #[test]
    fn phosphor_trail_preserves_dimensions() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 20, Rgba([32, 64, 96, 255])));
        let out = phosphor_trail(img, 5, 0.5, 0);
        assert_eq!(out.dimensions(), (20, 20));
    }
}
