use std::fmt;

use image::{DynamicImage, Rgba};
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;

/// Color manipulation effects: hue shift, contrast, invert, saturation, quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColorEffect {
    /// Rotate the hue of every pixel by a given angle (degrees).
    HueShift { degrees: f32 },
    /// Adjust contrast around the midpoint.
    Contrast { factor: f32 },
    /// Invert all colour channels.
    Invert,
    /// Scale the saturation of every pixel.
    Saturation { factor: f32 },
    /// Reduce the available colour palette (posterisation).
    ColorQuantization { levels: u8 },
    /// Remaps luminance to a custom colour gradient (e.g. sepia, duotone, synthwave).
    GradientMap {
        preset_idx: usize,
        stops: Vec<(f32, [u8; 3])>,
    },
    /// Swap the RGB channels into a different order for surreal colour palettes.
    ///
    /// `order`: 0=RGB (identity), 1=RBG, 2=GRB, 3=GBR, 4=BRG, 5=BGR.
    ChannelSwap { order: u8 },
    /// Apply Bayer-matrix or Floyd–Steinberg ordered dithering.
    ///
    /// `algorithm`: 0=Bayer 4×4, 1=Floyd–Steinberg.
    /// `levels`: number of quantisation levels per channel (2–8).
    Dither { algorithm: u8, levels: u8 },
}

/// A single gradient color stop: position (0.0–1.0) and RGB color.
pub type GradientStop = (f32, [u8; 3]);

/// Predefined color gradients for the GradientMap effect.
pub const GRADIENT_PRESETS: &[(&str, &[GradientStop])] = &[
    (
        "Synthwave",
        &[
            (0.0, [15, 5, 45]),
            (0.3, [70, 10, 110]),
            (0.6, [240, 20, 150]),
            (0.8, [255, 140, 50]),
            (1.0, [255, 255, 100]),
        ],
    ),
    (
        "Sepia",
        &[
            (0.0, [30, 15, 5]),
            (0.5, [140, 90, 50]),
            (1.0, [240, 210, 180]),
        ],
    ),
    (
        "Cyberpunk",
        &[
            (0.0, [5, 5, 20]),
            (0.2, [60, 0, 120]),
            (0.5, [255, 0, 180]),
            (0.8, [0, 255, 255]),
            (1.0, [255, 255, 255]),
        ],
    ),
    (
        "Night Vision",
        &[
            (0.0, [0, 10, 0]),
            (0.2, [0, 50, 0]),
            (0.8, [50, 255, 50]),
            (1.0, [200, 255, 200]),
        ],
    ),
    ("Custom", &[(0.0, [0, 0, 0]), (1.0, [255, 255, 255])]),
];

impl ColorEffect {
    /// Apply a colour transformation to a single pixel.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            ColorEffect::Invert => Rgba([255 - pixel[0], 255 - pixel[1], 255 - pixel[2], pixel[3]]),
            ColorEffect::Contrast { factor } => {
                let apply = |c: u8| -> u8 {
                    let f = (c as f32 / 255.0 - 0.5) * factor + 0.5;
                    (f.clamp(0.0, 1.0) * 255.0) as u8
                };
                Rgba([apply(pixel[0]), apply(pixel[1]), apply(pixel[2]), pixel[3]])
            }
            ColorEffect::GradientMap { stops, .. } => {
                if stops.is_empty() {
                    return pixel;
                }
                let luma = 0.2126 * (pixel[0] as f32 / 255.0)
                    + 0.7152 * (pixel[1] as f32 / 255.0)
                    + 0.0722 * (pixel[2] as f32 / 255.0);

                if stops.len() == 1 || luma <= stops[0].0 {
                    let c = stops[0].1;
                    return Rgba([c[0], c[1], c[2], pixel[3]]);
                }
                if luma >= stops.last().unwrap().0 {
                    let c = stops.last().unwrap().1;
                    return Rgba([c[0], c[1], c[2], pixel[3]]);
                }

                let mut lower = &stops[0];
                let mut upper = &stops[stops.len() - 1];
                for i in 0..stops.len() - 1 {
                    if luma >= stops[i].0 && luma <= stops[i + 1].0 {
                        lower = &stops[i];
                        upper = &stops[i + 1];
                        break;
                    }
                }

                let range = upper.0 - lower.0;
                let t = if range > 0.0 {
                    (luma - lower.0) / range
                } else {
                    0.0
                };

                let r = (lower.1[0] as f32 * (1.0 - t) + upper.1[0] as f32 * t) as u8;
                let g = (lower.1[1] as f32 * (1.0 - t) + upper.1[1] as f32 * t) as u8;
                let b = (lower.1[2] as f32 * (1.0 - t) + upper.1[2] as f32 * t) as u8;

                Rgba([r, g, b, pixel[3]])
            }
            ColorEffect::HueShift { degrees } => {
                let (r, g, b) = (
                    pixel[0] as f32 / 255.0,
                    pixel[1] as f32 / 255.0,
                    pixel[2] as f32 / 255.0,
                );
                let (h, s, l) = rgb_to_hsl(r, g, b);
                let h2 = (h + degrees / 360.0).fract();
                let (r2, g2, b2) = hsl_to_rgb(h2, s, l);
                Rgba([
                    (r2 * 255.0) as u8,
                    (g2 * 255.0) as u8,
                    (b2 * 255.0) as u8,
                    pixel[3],
                ])
            }
            ColorEffect::Saturation { factor } => {
                let (r, g, b) = (
                    pixel[0] as f32 / 255.0,
                    pixel[1] as f32 / 255.0,
                    pixel[2] as f32 / 255.0,
                );
                let (h, s, l) = rgb_to_hsl(r, g, b);
                let s2 = (s * factor).clamp(0.0, 1.0);
                let (r2, g2, b2) = hsl_to_rgb(h, s2, l);
                Rgba([
                    (r2 * 255.0) as u8,
                    (g2 * 255.0) as u8,
                    (b2 * 255.0) as u8,
                    pixel[3],
                ])
            }
            ColorEffect::ColorQuantization { levels } => {
                let levels = (*levels).max(2) as f32;
                let quantize = |c: u8| -> u8 {
                    let step = 255.0 / (levels - 1.0);
                    let idx = (c as f32 / step).round();
                    (idx * step).clamp(0.0, 255.0) as u8
                };
                Rgba([
                    quantize(pixel[0]),
                    quantize(pixel[1]),
                    quantize(pixel[2]),
                    pixel[3],
                ])
            }
            ColorEffect::ChannelSwap { order } => {
                let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
                let (nr, ng, nb) = match order {
                    0 => (r, g, b), // RGB (identity)
                    1 => (r, b, g), // RBG
                    2 => (g, r, b), // GRB
                    3 => (g, b, r), // GBR
                    4 => (b, r, g), // BRG
                    _ => (b, g, r), // BGR (5)
                };
                Rgba([nr, ng, nb, a])
            }
            // Dither requires full-image context; apply_pixel is a no-op fallback.
            ColorEffect::Dither { .. } => pixel,
        }
    }

    /// Apply a full-image colour transformation.
    ///
    /// Dither variants require sequential per-pixel passes with access to the full
    /// image buffer (Floyd–Steinberg error diffusion) or row/column coordinates
    /// (Bayer ordered dithering).  All other colour effects delegate to
    /// [`apply_pixel`][Self::apply_pixel] via [`apply_per_pixel`][super::apply_per_pixel].
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            ColorEffect::Dither { algorithm, levels } => dither_image(img, *algorithm, *levels),
            _ => super::apply_per_pixel(img, |p, _x, _y| self.apply_pixel(p)),
        }
    }

    /// Return descriptors for all editable numeric parameters.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            ColorEffect::Invert => vec![],
            ColorEffect::GradientMap { preset_idx, stops } => {
                let mut params = vec![ParamDescriptor {
                    name: "preset",
                    value: *preset_idx as f32,
                    min: 0.0,
                    max: (GRADIENT_PRESETS.len() - 1) as f32,
                }];

                // If it's the "Custom" preset (last one), allow editing colors.
                if *preset_idx == GRADIENT_PRESETS.len() - 1 && stops.len() >= 2 {
                    params.push(ParamDescriptor {
                        name: "r1",
                        value: stops[0].1[0] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                    params.push(ParamDescriptor {
                        name: "g1",
                        value: stops[0].1[1] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                    params.push(ParamDescriptor {
                        name: "b1",
                        value: stops[0].1[2] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                    params.push(ParamDescriptor {
                        name: "r2",
                        value: stops[1].1[0] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                    params.push(ParamDescriptor {
                        name: "g2",
                        value: stops[1].1[1] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                    params.push(ParamDescriptor {
                        name: "b2",
                        value: stops[1].1[2] as f32,
                        min: 0.0,
                        max: 255.0,
                    });
                }
                params
            }
            ColorEffect::HueShift { degrees } => vec![ParamDescriptor {
                name: "degrees",
                value: *degrees,
                min: 0.0,
                max: 360.0,
            }],
            ColorEffect::Contrast { factor } => vec![ParamDescriptor {
                name: "factor",
                value: *factor,
                min: 0.1,
                max: 3.0,
            }],
            ColorEffect::Saturation { factor } => vec![ParamDescriptor {
                name: "factor",
                value: *factor,
                min: 0.0,
                max: 2.0,
            }],
            ColorEffect::ColorQuantization { levels } => vec![ParamDescriptor {
                name: "levels",
                value: *levels as f32,
                min: 2.0,
                max: 16.0,
            }],
            ColorEffect::ChannelSwap { order } => vec![ParamDescriptor {
                name: "order",
                value: *order as f32,
                min: 0.0,
                max: 5.0,
            }],
            ColorEffect::Dither { algorithm, levels } => vec![
                ParamDescriptor {
                    name: "algorithm",
                    value: *algorithm as f32,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "levels",
                    value: *levels as f32,
                    min: 2.0,
                    max: 8.0,
                },
            ],
        }
    }

    /// Rebuild this variant with new parameter values (clamped to valid ranges).
    pub fn apply_params(&self, values: &[f32]) -> ColorEffect {
        let get = |i: usize, fallback: f32| values.get(i).copied().unwrap_or(fallback);
        match self {
            ColorEffect::Invert => ColorEffect::Invert,
            ColorEffect::GradientMap { preset_idx, stops } => {
                let new_preset_idx = get(0, *preset_idx as f32) as usize;
                if new_preset_idx != *preset_idx {
                    ColorEffect::GradientMap {
                        preset_idx: new_preset_idx,
                        stops: GRADIENT_PRESETS[new_preset_idx].1.to_vec(),
                    }
                } else if new_preset_idx == GRADIENT_PRESETS.len() - 1 {
                    let mut new_stops = stops.clone();
                    if new_stops.len() >= 2 && values.len() >= 7 {
                        new_stops[0].1 = [get(1, 0.0) as u8, get(2, 0.0) as u8, get(3, 0.0) as u8];
                        new_stops[1].1 = [get(4, 0.0) as u8, get(5, 0.0) as u8, get(6, 0.0) as u8];
                    }
                    ColorEffect::GradientMap {
                        preset_idx: new_preset_idx,
                        stops: new_stops,
                    }
                } else {
                    ColorEffect::GradientMap {
                        preset_idx: *preset_idx,
                        stops: stops.clone(),
                    }
                }
            }
            ColorEffect::HueShift { degrees } => ColorEffect::HueShift {
                degrees: get(0, *degrees),
            },
            ColorEffect::Contrast { factor } => ColorEffect::Contrast {
                factor: get(0, *factor),
            },
            ColorEffect::Saturation { factor } => ColorEffect::Saturation {
                factor: get(0, *factor),
            },
            ColorEffect::ColorQuantization { levels } => ColorEffect::ColorQuantization {
                levels: get(0, *levels as f32) as u8,
            },
            ColorEffect::ChannelSwap { order } => ColorEffect::ChannelSwap {
                order: get(0, *order as f32).clamp(0.0, 5.0) as u8,
            },
            ColorEffect::Dither { algorithm, levels } => ColorEffect::Dither {
                algorithm: get(0, *algorithm as f32).clamp(0.0, 1.0) as u8,
                levels: get(1, *levels as f32).clamp(2.0, 8.0) as u8,
            },
        }
    }

    /// Returns the variant name (e.g. `"HueShift"`, `"Invert"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            ColorEffect::Invert => "Invert",
            ColorEffect::GradientMap { .. } => "GradientMap",
            ColorEffect::HueShift { .. } => "HueShift",
            ColorEffect::Contrast { .. } => "Contrast",
            ColorEffect::Saturation { .. } => "Saturation",
            ColorEffect::ColorQuantization { .. } => "ColorQuantization",
            ColorEffect::ChannelSwap { .. } => "ChannelSwap",
            ColorEffect::Dither { .. } => "Dither",
        }
    }
}

impl fmt::Display for ColorEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColorEffect::Invert => write!(f, "Invert"),
            ColorEffect::GradientMap { preset_idx, .. } => {
                let name = GRADIENT_PRESETS
                    .get(*preset_idx)
                    .map(|(n, _)| *n)
                    .unwrap_or("Unknown");
                write!(f, "Gradient {name}")
            }
            ColorEffect::HueShift { degrees } => write!(f, "HueShift {degrees:.0}°"),
            ColorEffect::Contrast { factor } => write!(f, "Contrast ×{factor:.2}"),
            ColorEffect::Saturation { factor } => write!(f, "Saturation ×{factor:.2}"),
            ColorEffect::ColorQuantization { levels } => write!(f, "Quantize {levels}"),
            ColorEffect::ChannelSwap { order } => {
                let label = match order {
                    0 => "RGB",
                    1 => "RBG",
                    2 => "GRB",
                    3 => "GBR",
                    4 => "BRG",
                    _ => "BGR",
                };
                write!(f, "ChannelSwap {label}")
            }
            ColorEffect::Dither { algorithm, levels } => {
                if *algorithm == 0 {
                    write!(f, "Dither Bayer {levels}")
                } else {
                    write!(f, "Dither FS {levels}")
                }
            }
        }
    }
}

// ── HSL ↔ RGB helpers ───────────────────────────────────────────────────────

/// Convert sRGB (0..1) to HSL (each 0..1).
fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0
        }
        h / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0
    }
    if t > 1.0 {
        t -= 1.0
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Convert HSL (each 0..1) to sRGB (0..1).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    (
        hue_to_rgb(p, q, h + 1.0 / 3.0),
        hue_to_rgb(p, q, h),
        hue_to_rgb(p, q, h - 1.0 / 3.0),
    )
}

// ── Dither helpers ───────────────────────────────────────────────────────────

/// Apply Bayer-matrix ordered dithering or Floyd–Steinberg error diffusion.
fn dither_image(img: DynamicImage, algorithm: u8, levels: u8) -> DynamicImage {
    let levels = levels.clamp(2, 8) as f32;
    let step = 255.0 / (levels - 1.0);
    let quantize = |c: f32| -> u8 {
        let idx = (c / step).round();
        (idx * step).clamp(0.0, 255.0) as u8
    };

    if algorithm == 0 {
        // Bayer 4×4 ordered dithering.
        let bayer: [[f32; 4]; 4] = [
            [0.0, 8.0, 2.0, 10.0],
            [12.0, 4.0, 14.0, 6.0],
            [3.0, 11.0, 1.0, 9.0],
            [15.0, 7.0, 13.0, 5.0],
        ];
        super::apply_per_pixel(img, move |p, x, y| {
            let threshold = bayer[(y % 4) as usize][(x % 4) as usize] / 16.0 - 0.5;
            let spread = threshold / levels;
            let r = quantize(p[0] as f32 + spread * 255.0);
            let g = quantize(p[1] as f32 + spread * 255.0);
            let b = quantize(p[2] as f32 + spread * 255.0);
            Rgba([r, g, b, p[3]])
        })
    } else {
        // Floyd–Steinberg error diffusion (sequential, row-major).
        let width = img.width() as usize;
        let height = img.height() as usize;
        let mut rgba = img.into_rgba8();
        // Work on f32 error buffer (R, G, B per pixel).
        let mut errs: Vec<[f32; 3]> = vec![[0.0; 3]; width * height];

        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                let pix = rgba.get_pixel(x as u32, y as u32);
                let r_in = pix[0] as f32 + errs[idx][0];
                let g_in = pix[1] as f32 + errs[idx][1];
                let b_in = pix[2] as f32 + errs[idx][2];
                let r_out = quantize(r_in);
                let g_out = quantize(g_in);
                let b_out = quantize(b_in);
                rgba.put_pixel(x as u32, y as u32, Rgba([r_out, g_out, b_out, pix[3]]));
                let er = r_in - r_out as f32;
                let eg = g_in - g_out as f32;
                let eb = b_in - b_out as f32;
                // Distribute error to right, below-left, below, below-right.
                let distribute = |errs: &mut Vec<[f32; 3]>, nx: usize, ny: usize, w: f32| {
                    if nx < width && ny < height {
                        let ni = ny * width + nx;
                        errs[ni][0] += er * w;
                        errs[ni][1] += eg * w;
                        errs[ni][2] += eb * w;
                    }
                };
                distribute(&mut errs, x + 1, y, 7.0 / 16.0);
                if x > 0 {
                    distribute(&mut errs, x - 1, y + 1, 3.0 / 16.0);
                }
                distribute(&mut errs, x, y + 1, 5.0 / 16.0);
                distribute(&mut errs, x + 1, y + 1, 1.0 / 16.0);
            }
        }
        DynamicImage::ImageRgba8(rgba)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(r: u8, g: u8, b: u8) -> Rgba<u8> {
        Rgba([r, g, b, 255])
    }

    #[test]
    fn invert_round_trips() {
        let p = px(100, 150, 200);
        let e = ColorEffect::Invert;
        let q = e.apply_pixel(e.apply_pixel(p));
        assert_eq!(p, q);
    }

    #[test]
    fn contrast_identity_at_one() {
        let p = px(128, 64, 200);
        let out = ColorEffect::Contrast { factor: 1.0 }.apply_pixel(p);
        assert_eq!(p, out);
    }

    #[test]
    fn hue_shift_360_is_identity() {
        let p = px(200, 100, 50);
        let out = ColorEffect::HueShift { degrees: 360.0 }.apply_pixel(p);
        // Allow ±1 rounding error per channel.
        for i in 0..3 {
            let diff = (out[i] as i16 - p[i] as i16).abs();
            assert!(diff <= 1, "channel {i}: expected ~{}, got {}", p[i], out[i]);
        }
    }

    #[test]
    fn saturation_zero_gives_grey() {
        let p = px(200, 100, 50);
        let out = ColorEffect::Saturation { factor: 0.0 }.apply_pixel(p);
        // All channels should be equal (grey).
        assert_eq!(out[0], out[1], "R should equal G for grey");
        assert_eq!(out[1], out[2], "G should equal B for grey");
    }

    #[test]
    fn quantization_two_levels() {
        // With levels=2, each channel should be either 0 or 255.
        let e = ColorEffect::ColorQuantization { levels: 2 };
        for v in [0u8, 64, 128, 192, 255] {
            let out = e.apply_pixel(px(v, v, v));
            assert!(
                out[0] == 0 || out[0] == 255,
                "Expected 0 or 255, got {}",
                out[0]
            );
        }
    }

    #[test]
    fn channel_swap_identity() {
        let p = px(100, 150, 200);
        let out = ColorEffect::ChannelSwap { order: 0 }.apply_pixel(p);
        assert_eq!(p, out);
    }

    #[test]
    fn channel_swap_bgr() {
        let p = px(100, 150, 200);
        let out = ColorEffect::ChannelSwap { order: 5 }.apply_pixel(p);
        assert_eq!(out[0], 200, "R should be original B");
        assert_eq!(out[1], 150, "G unchanged");
        assert_eq!(out[2], 100, "B should be original R");
        assert_eq!(out[3], 255);
    }

    #[test]
    fn dither_bayer_preserves_dimensions() {
        use image::{DynamicImage, ImageBuffer};
        let img: DynamicImage =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(16, 16, Rgba([128u8, 64, 200, 255])));
        let out = ColorEffect::Dither {
            algorithm: 0,
            levels: 4,
        }
        .apply_image(img);
        assert_eq!(out.width(), 16);
        assert_eq!(out.height(), 16);
    }

    #[test]
    fn dither_bayer_two_levels_binary() {
        use image::{DynamicImage, ImageBuffer};
        // With levels=2, each channel output must be 0 or 255.
        let img: DynamicImage =
            DynamicImage::ImageRgba8(ImageBuffer::from_pixel(8, 8, Rgba([128u8, 128, 128, 255])));
        let out = ColorEffect::Dither {
            algorithm: 0,
            levels: 2,
        }
        .apply_image(img)
        .into_rgba8();
        for pixel in out.pixels() {
            for &c in &pixel.0[..3] {
                assert!(c == 0 || c == 255, "Expected 0 or 255, got {c}");
            }
        }
    }
}
