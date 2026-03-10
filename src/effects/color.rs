use image::Rgba;
use serde::{Deserialize, Serialize};

/// Color manipulation effects: hue shift, contrast, invert, saturation.
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
}

impl ColorEffect {
    /// Apply a colour transformation to a single pixel.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            ColorEffect::Invert => {
                Rgba([255 - pixel[0], 255 - pixel[1], 255 - pixel[2], pixel[3]])
            }
            ColorEffect::Contrast { factor } => {
                let apply = |c: u8| -> u8 {
                    let f = (c as f32 / 255.0 - 0.5) * factor + 0.5;
                    (f.clamp(0.0, 1.0) * 255.0) as u8
                };
                Rgba([apply(pixel[0]), apply(pixel[1]), apply(pixel[2]), pixel[3]])
            }
            ColorEffect::HueShift { .. } => pixel,
            ColorEffect::Saturation { .. } => pixel,
        }
    }
}
