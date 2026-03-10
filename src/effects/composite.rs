use image::Rgba;
use serde::{Deserialize, Serialize};

/// Compositing effects: image blend and crop/rect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompositeEffect {
    /// Blend the current image with another at the given opacity.
    ImageBlend { opacity: f32 },
    /// Crop the image to the given rectangle.
    CropRect {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

impl CompositeEffect {
    /// Apply per-pixel compositing transformation.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            CompositeEffect::ImageBlend { .. } => pixel,
            CompositeEffect::CropRect { .. } => pixel,
        }
    }
}
