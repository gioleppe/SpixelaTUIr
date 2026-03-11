use image::DynamicImage;
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
    /// Apply this effect to a full image buffer.
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            CompositeEffect::CropRect {
                x,
                y,
                width,
                height,
            } => {
                // Clamp to the image bounds to prevent panics.
                let img_w = img.width();
                let img_h = img.height();
                let cx = (*x).min(img_w);
                let cy = (*y).min(img_h);
                let cw = (*width).min(img_w.saturating_sub(cx));
                let ch = (*height).min(img_h.saturating_sub(cy));
                if cw == 0 || ch == 0 {
                    return img;
                }
                img.crop_imm(cx, cy, cw, ch)
            }
            // ImageBlend without a secondary asset is a no-op.
            CompositeEffect::ImageBlend { .. } => img,
        }
    }
}
