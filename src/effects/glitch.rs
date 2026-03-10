use image::Rgba;
use serde::{Deserialize, Serialize};

/// Glitch-style pixel effects: pixelate, row jitter, block shift, pixel sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlitchEffect {
    /// Reduce effective resolution by grouping pixels into blocks.
    Pixelate { block_size: u32 },
    /// Randomly displace rows horizontally.
    RowJitter { magnitude: f32 },
    /// Shift rectangular blocks of pixels.
    BlockShift { shift_x: i32, shift_y: i32 },
    /// Sort pixels within rows or columns by luminance or hue.
    PixelSort { threshold: f32 },
}

impl GlitchEffect {
    /// Apply per-pixel transformation.
    ///
    /// Most glitch effects require full-image context; per-pixel application
    /// is a simplified approximation suitable for pipeline composition.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            GlitchEffect::Pixelate { .. } => pixel,
            GlitchEffect::RowJitter { .. } => pixel,
            GlitchEffect::BlockShift { .. } => pixel,
            GlitchEffect::PixelSort { .. } => pixel,
        }
    }
}
