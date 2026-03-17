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
    /// Mirror image stripes in horizontal or vertical bands.
    MirrorSlice {
        orientation: u8,
        slice_width: u32,
        pattern: u8,
    },
    /// Detect edges and blend a configurable glow tint.
    EdgeGlow {
        edge_thresh: f32,
        glow_color_r: u8,
        glow_color_g: u8,
        glow_color_b: u8,
        glow_strength: f32,
        blur_radius: u32,
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
            CompositeEffect::MirrorSlice {
                orientation,
                slice_width,
                pattern,
            } => mirror_slice(img, *orientation, *slice_width, *pattern),
            CompositeEffect::EdgeGlow {
                edge_thresh,
                glow_color_r,
                glow_color_g,
                glow_color_b,
                glow_strength,
                blur_radius,
            } => edge_glow(
                img,
                *edge_thresh,
                *glow_color_r,
                *glow_color_g,
                *glow_color_b,
                *glow_strength,
                *blur_radius,
            ),
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
            CompositeEffect::MirrorSlice {
                orientation,
                slice_width,
                pattern,
            } => vec![
                ParamDescriptor {
                    name: "orientation",
                    value: *orientation as f32,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "slice_width",
                    value: *slice_width as f32,
                    min: 1.0,
                    max: 200.0,
                },
                ParamDescriptor {
                    name: "pattern",
                    value: *pattern as f32,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            CompositeEffect::EdgeGlow {
                edge_thresh,
                glow_color_r,
                glow_color_g,
                glow_color_b,
                glow_strength,
                blur_radius,
            } => vec![
                ParamDescriptor {
                    name: "edge_thresh",
                    value: *edge_thresh,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "glow_r",
                    value: *glow_color_r as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "glow_g",
                    value: *glow_color_g as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "glow_b",
                    value: *glow_color_b as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "glow_strength",
                    value: *glow_strength,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "blur_radius",
                    value: *blur_radius as f32,
                    min: 0.0,
                    max: 5.0,
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
            CompositeEffect::MirrorSlice {
                orientation,
                slice_width,
                pattern,
            } => CompositeEffect::MirrorSlice {
                orientation: get(0, *orientation as f32) as u8,
                slice_width: get(1, *slice_width as f32) as u32,
                pattern: get(2, *pattern as f32) as u8,
            },
            CompositeEffect::EdgeGlow {
                edge_thresh,
                glow_color_r,
                glow_color_g,
                glow_color_b,
                glow_strength,
                blur_radius,
            } => CompositeEffect::EdgeGlow {
                edge_thresh: get(0, *edge_thresh),
                glow_color_r: get(1, *glow_color_r as f32) as u8,
                glow_color_g: get(2, *glow_color_g as f32) as u8,
                glow_color_b: get(3, *glow_color_b as f32) as u8,
                glow_strength: get(4, *glow_strength),
                blur_radius: get(5, *blur_radius as f32) as u32,
            },
        }
    }

    /// Returns the variant name (e.g. `"ImageBlend"`, `"CropRect"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            CompositeEffect::ImageBlend { .. } => "ImageBlend",
            CompositeEffect::CropRect { .. } => "CropRect",
            CompositeEffect::MirrorSlice { .. } => "MirrorSlice",
            CompositeEffect::EdgeGlow { .. } => "EdgeGlow",
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
            CompositeEffect::MirrorSlice {
                orientation,
                slice_width,
                ..
            } => {
                let axis = if *orientation == 0 { " H" } else { " V" };
                write!(f, "MirrorSlice {slice_width}px{axis}")
            }
            CompositeEffect::EdgeGlow {
                edge_thresh,
                glow_strength,
                ..
            } => write!(f, "EdgeGlow t={edge_thresh:.2} s={glow_strength:.2}"),
        }
    }
}

fn mirror_slice(img: DynamicImage, orientation: u8, slice_width: u32, pattern: u8) -> DynamicImage {
    let src = img.into_rgba8();
    let (width, height) = src.dimensions();
    let mut out = src.clone();
    let band = slice_width.max(1);

    if orientation == 0 {
        let band_count = height.div_ceil(band);
        for b in 0..band_count {
            let start = b * band;
            let end = (start + band).min(height);
            let cur_h = end.saturating_sub(start);
            if cur_h == 0 {
                continue;
            }
            match pattern {
                1 if b > 0 => {
                    let prev_start = (b - 1) * band;
                    let prev_end = (prev_start + band).min(height);
                    let prev_h = prev_end.saturating_sub(prev_start);
                    for y in start..end {
                        let rel =
                            ((y - start) as usize).min(prev_h.saturating_sub(1) as usize) as u32;
                        let src_y = prev_start + prev_h.saturating_sub(1) - rel;
                        for x in 0..width {
                            let p = *src.get_pixel(x, src_y);
                            out.put_pixel(x, y, p);
                        }
                    }
                }
                _ if b % 2 == 1 => {
                    for y in start..end {
                        let rel = y - start;
                        let src_y = start + cur_h.saturating_sub(1) - rel;
                        for x in 0..width {
                            let p = *src.get_pixel(x, src_y);
                            out.put_pixel(x, y, p);
                        }
                    }
                }
                _ => {}
            }
        }
    } else {
        let band_count = width.div_ceil(band);
        for b in 0..band_count {
            let start = b * band;
            let end = (start + band).min(width);
            let cur_w = end.saturating_sub(start);
            if cur_w == 0 {
                continue;
            }
            match pattern {
                1 if b > 0 => {
                    let prev_start = (b - 1) * band;
                    let prev_end = (prev_start + band).min(width);
                    let prev_w = prev_end.saturating_sub(prev_start);
                    for x in start..end {
                        let rel =
                            ((x - start) as usize).min(prev_w.saturating_sub(1) as usize) as u32;
                        let src_x = prev_start + prev_w.saturating_sub(1) - rel;
                        for y in 0..height {
                            let p = *src.get_pixel(src_x, y);
                            out.put_pixel(x, y, p);
                        }
                    }
                }
                _ if b % 2 == 1 => {
                    for x in start..end {
                        let rel = x - start;
                        let src_x = start + cur_w.saturating_sub(1) - rel;
                        for y in 0..height {
                            let p = *src.get_pixel(src_x, y);
                            out.put_pixel(x, y, p);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    DynamicImage::ImageRgba8(out)
}

fn edge_glow(
    img: DynamicImage,
    edge_thresh: f32,
    glow_color_r: u8,
    glow_color_g: u8,
    glow_color_b: u8,
    glow_strength: f32,
    blur_radius: u32,
) -> DynamicImage {
    let src = img.into_rgba8();
    let (w, h) = src.dimensions();
    let mut out = src.clone();
    if w == 0 || h == 0 {
        return DynamicImage::ImageRgba8(out);
    }

    let idx = |x: u32, y: u32| -> usize { (y * w + x) as usize };
    let mut gray = vec![0.0_f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = src.get_pixel(x, y);
            gray[idx(x, y)] =
                (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
        }
    }

    let mut mag = vec![0.0_f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let xm1 = x.saturating_sub(1);
            let xp1 = (x + 1).min(w - 1);
            let ym1 = y.saturating_sub(1);
            let yp1 = (y + 1).min(h - 1);

            let tl = gray[idx(xm1, ym1)];
            let tc = gray[idx(x, ym1)];
            let tr = gray[idx(xp1, ym1)];
            let ml = gray[idx(xm1, y)];
            let mr = gray[idx(xp1, y)];
            let bl = gray[idx(xm1, yp1)];
            let bc = gray[idx(x, yp1)];
            let br = gray[idx(xp1, yp1)];

            let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
            let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
            mag[idx(x, y)] = (gx.mul_add(gx, gy * gy)).sqrt().min(4.0) / 4.0;
        }
    }

    let edge_map = if blur_radius == 0 {
        mag
    } else {
        let r = blur_radius;
        let mut blurred = vec![0.0_f32; (w * h) as usize];
        for y in 0..h {
            let y0 = y.saturating_sub(r);
            let y1 = (y + r).min(h - 1);
            for x in 0..w {
                let x0 = x.saturating_sub(r);
                let x1 = (x + r).min(w - 1);
                let mut sum = 0.0_f32;
                let mut count = 0_u32;
                for yy in y0..=y1 {
                    for xx in x0..=x1 {
                        sum += mag[idx(xx, yy)];
                        count += 1;
                    }
                }
                blurred[idx(x, y)] = if count == 0 { 0.0 } else { sum / count as f32 };
            }
        }
        blurred
    };

    let thresh = edge_thresh.clamp(0.0, 1.0);
    let strength = glow_strength.clamp(0.0, 1.0);
    let background_darkening = 1.0 - strength * 0.8;
    for y in 0..h {
        for x in 0..w {
            let source = src.get_pixel(x, y);
            let magnitude = edge_map[idx(x, y)];
            let edge_factor = if magnitude > thresh {
                magnitude.clamp(0.0, 1.0)
            } else {
                0.0
            };

            let bg_factor = background_darkening + (1.0 - background_darkening) * edge_factor;
            let mut r = source[0] as f32 * bg_factor;
            let mut g = source[1] as f32 * bg_factor;
            let mut b = source[2] as f32 * bg_factor;

            if edge_factor > 0.0 {
                let w_glow = (strength * edge_factor).clamp(0.0, 1.0);
                r = r * (1.0 - w_glow) + glow_color_r as f32 * w_glow;
                g = g * (1.0 - w_glow) + glow_color_g as f32 * w_glow;
                b = b * (1.0 - w_glow) + glow_color_b as f32 * w_glow;
            }

            let p = out.get_pixel_mut(x, y);
            p[0] = r.clamp(0.0, 255.0) as u8;
            p[1] = g.clamp(0.0, 255.0) as u8;
            p[2] = b.clamp(0.0, 255.0) as u8;
        }
    }

    DynamicImage::ImageRgba8(out)
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};

    use super::CompositeEffect;

    #[test]
    fn mirror_slice_preserves_dimensions() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(24, 12, Rgba([10, 20, 30, 255])));
        let out = CompositeEffect::MirrorSlice {
            orientation: 0,
            slice_width: 3,
            pattern: 0,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (24, 12));
    }

    #[test]
    fn mirror_slice_slice_width_full_alternate_is_identity() {
        let mut img = RgbaImage::new(7, 5);
        for y in 0..5 {
            for x in 0..7 {
                img.put_pixel(
                    x,
                    y,
                    Rgba([(x * 17) as u8, (y * 33) as u8, (x + y) as u8, 255]),
                );
            }
        }
        let input = DynamicImage::ImageRgba8(img.clone());
        let out = CompositeEffect::MirrorSlice {
            orientation: 0,
            slice_width: 5,
            pattern: 0,
        }
        .apply_image(input)
        .into_rgba8();
        assert_eq!(out, img);
    }

    #[test]
    fn edge_glow_preserves_dimensions() {
        let img =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(18, 11, Rgba([100, 110, 120, 255])));
        let out = CompositeEffect::EdgeGlow {
            edge_thresh: 0.1,
            glow_color_r: 0,
            glow_color_g: 255,
            glow_color_b: 255,
            glow_strength: 0.8,
            blur_radius: 1,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (18, 11));
    }
}
