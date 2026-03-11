use std::fmt;

use image::DynamicImage;
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;

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

    /// Return descriptors for all editable numeric parameters.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            CompositeEffect::ImageBlend { opacity } => vec![ParamDescriptor {
                name: "opacity",
                value: *opacity,
                min: 0.0,
                max: 1.0,
            }],
            CompositeEffect::CropRect {
                x,
                y,
                width,
                height,
            } => vec![
                ParamDescriptor {
                    name: "x",
                    value: *x as f32,
                    min: 0.0,
                    max: 4096.0,
                },
                ParamDescriptor {
                    name: "y",
                    value: *y as f32,
                    min: 0.0,
                    max: 4096.0,
                },
                ParamDescriptor {
                    name: "width",
                    value: *width as f32,
                    min: 1.0,
                    max: 4096.0,
                },
                ParamDescriptor {
                    name: "height",
                    value: *height as f32,
                    min: 1.0,
                    max: 4096.0,
                },
            ],
        }
    }

    /// Rebuild this variant with new parameter values (clamped to valid ranges).
    pub fn apply_params(&self, values: &[f32]) -> CompositeEffect {
        let get = |i: usize, fallback: f32| values.get(i).copied().unwrap_or(fallback);
        match self {
            CompositeEffect::ImageBlend { opacity } => CompositeEffect::ImageBlend {
                opacity: get(0, *opacity),
            },
            CompositeEffect::CropRect {
                x,
                y,
                width,
                height,
            } => CompositeEffect::CropRect {
                x: get(0, *x as f32) as u32,
                y: get(1, *y as f32) as u32,
                width: get(2, *width as f32) as u32,
                height: get(3, *height as f32) as u32,
            },
        }
    }

    /// Returns the variant name (e.g. `"ImageBlend"`, `"CropRect"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            CompositeEffect::ImageBlend { .. } => "ImageBlend",
            CompositeEffect::CropRect { .. } => "CropRect",
        }
    }
}

impl fmt::Display for CompositeEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompositeEffect::ImageBlend { opacity } => write!(f, "Blend {opacity:.0}%"),
            CompositeEffect::CropRect {
                x,
                y,
                width,
                height,
            } => write!(f, "Crop {x},{y} {width}×{height}"),
        }
    }
}
