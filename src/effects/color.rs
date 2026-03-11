use image::Rgba;
use serde::{Deserialize, Serialize};

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
}

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
}
