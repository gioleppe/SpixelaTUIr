use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
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
    /// Sort pixels within rows by luminance.
    PixelSort { threshold: f32 },
}

impl GlitchEffect {
    /// Apply this effect to an entire image, enabling full spatial context.
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            GlitchEffect::Pixelate { block_size } => pixelate(img, *block_size),
            GlitchEffect::RowJitter { magnitude } => row_jitter(img, *magnitude),
            GlitchEffect::BlockShift { shift_x, shift_y } => block_shift(img, *shift_x, *shift_y),
            GlitchEffect::PixelSort { threshold } => pixel_sort(img, *threshold),
        }
    }
}

// ── Pixelate ─────────────────────────────────────────────────────────────────

fn pixelate(img: DynamicImage, block_size: u32) -> DynamicImage {
    if block_size <= 1 {
        return img;
    }
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let w_usize = w as usize;
    let h_usize = h as usize;
    let bsize = block_size as usize;
    let row_stride = w_usize * 4;
    let strip_stride = bsize * row_stride;
    let raw = rgba.into_raw();
    let mut out_raw = vec![0u8; raw.len()];

    // Each horizontal strip of `block_size` rows is fully independent.
    // The tight inner loops over contiguous byte slices allow LLVM to
    // auto-vectorise this with SIMD when compiling with optimisations.
    out_raw
        .chunks_mut(strip_stride)
        .enumerate()
        .for_each(|(strip_idx, strip)| {
            let by = strip_idx * bsize;
            let actual_bh = bsize.min(h_usize - by);

            for bx in (0..w_usize).step_by(bsize) {
                let actual_bw = bsize.min(w_usize - bx);
                let cnt = (actual_bh * actual_bw) as u64;

                // Compute average colour across the block.
                let (mut sr, mut sg, mut sb, mut sa) = (0u64, 0u64, 0u64, 0u64);
                for dy in 0..actual_bh {
                    for dx in 0..actual_bw {
                        let idx = ((by + dy) * w_usize + (bx + dx)) * 4;
                        sr += raw[idx] as u64;
                        sg += raw[idx + 1] as u64;
                        sb += raw[idx + 2] as u64;
                        sa += raw[idx + 3] as u64;
                    }
                }
                let avg = [
                    (sr / cnt) as u8,
                    (sg / cnt) as u8,
                    (sb / cnt) as u8,
                    (sa / cnt) as u8,
                ];

                // Fill every pixel in the block with the average colour.
                for dy in 0..actual_bh {
                    for dx in 0..actual_bw {
                        let dst = dy * row_stride + (bx + dx) * 4;
                        strip[dst..dst + 4].copy_from_slice(&avg);
                    }
                }
            }
        });

    let out = image::ImageBuffer::from_raw(w, h, out_raw).expect("buffer size mismatch");
    DynamicImage::ImageRgba8(out)
}

// ── Row Jitter ────────────────────────────────────────────────────────────────

fn row_jitter(img: DynamicImage, magnitude: f32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let max_shift = (w as f32 * magnitude.abs()) as i32;

    // Collect all rows as independent slices.
    let rows: Vec<Vec<image::Rgba<u8>>> = (0..h)
        .map(|y| (0..w).map(|x| *rgba.get_pixel(x, y)).collect())
        .collect();

    // Shift each row with a deterministic per-row hash.
    let shifted_rows: Vec<Vec<image::Rgba<u8>>> = rows
        .iter()
        .enumerate()
        .map(|(y, row)| {
            let hash = (y as u32).wrapping_mul(2654435761) ^ (y as u32).wrapping_mul(2246822519);
            let n = ((hash as f32).sin() * 43758.5453).fract();
            let shift = ((n * 2.0 - 1.0) * max_shift as f32) as i32;
            (0..w)
                .map(|x| {
                    let src_x = ((x as i32 + shift).rem_euclid(w as i32)) as usize;
                    row[src_x]
                })
                .collect()
        })
        .collect();

    let mut out = image::ImageBuffer::new(w, h);
    for (y, row) in shifted_rows.iter().enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            out.put_pixel(x as u32, y as u32, *pixel);
        }
    }
    DynamicImage::ImageRgba8(out)
}

// ── Block Shift ───────────────────────────────────────────────────────────────

fn block_shift(img: DynamicImage, shift_x: i32, shift_y: i32) -> DynamicImage {
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let w_usize = w as usize;
    let raw = rgba.into_raw();
    let mut out_raw = vec![0u8; raw.len()];

    // Each destination row is filled from a (possibly different) source row.
    out_raw
        .chunks_mut(w_usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = ((y as i32 + shift_y).rem_euclid(h as i32)) as usize;
            for x in 0..w_usize {
                let src_x = ((x as i32 + shift_x).rem_euclid(w as i32)) as usize;
                let src_idx = (src_y * w_usize + src_x) * 4;
                let dst_idx = x * 4;
                row[dst_idx..dst_idx + 4].copy_from_slice(&raw[src_idx..src_idx + 4]);
            }
        });

    let out = image::ImageBuffer::from_raw(w, h, out_raw).expect("buffer size mismatch");
    DynamicImage::ImageRgba8(out)
}

// ── Pixel Sort ────────────────────────────────────────────────────────────────

fn luminance(p: &Rgba<u8>) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn pixel_sort(img: DynamicImage, threshold: f32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let thresh = threshold * 255.0;

    // Process each row sequentially, sorting above-threshold pixel runs by luminance.
    let sorted_rows: Vec<Vec<Rgba<u8>>> = (0..h)
        .map(|y| (0..w).map(|x| *rgba.get_pixel(x, y)).collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|row| {
            let mut sorted = row.clone();
            let mut seg_start: Option<usize> = None;
            let n = row.len();
            for x in 0..=n {
                let above = x < n && luminance(&row[x]) > thresh;
                if above {
                    if seg_start.is_none() {
                        seg_start = Some(x);
                    }
                } else if let Some(start) = seg_start.take() {
                    let segment = &mut sorted[start..x];
                    segment.sort_by(|a, b| luminance(a).partial_cmp(&luminance(b)).unwrap());
                }
            }
            sorted
        })
        .collect();

    let mut out = ImageBuffer::new(w, h);
    for (y, row) in sorted_rows.iter().enumerate() {
        for (x, pixel) in row.iter().enumerate() {
            out.put_pixel(x as u32, y as u32, *pixel);
        }
    }
    DynamicImage::ImageRgba8(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    fn solid_image(w: u32, h: u32, color: Rgba<u8>) -> DynamicImage {
        let buf = ImageBuffer::from_pixel(w, h, color);
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn pixelate_preserves_dimensions() {
        let img = solid_image(100, 80, Rgba([255, 0, 0, 255]));
        let out = GlitchEffect::Pixelate { block_size: 10 }.apply_image(img);
        assert_eq!(out.dimensions(), (100, 80));
    }

    #[test]
    fn row_jitter_preserves_dimensions() {
        let img = solid_image(50, 40, Rgba([0, 255, 0, 255]));
        let out = GlitchEffect::RowJitter { magnitude: 0.2 }.apply_image(img);
        assert_eq!(out.dimensions(), (50, 40));
    }

    #[test]
    fn pixel_sort_preserves_dimensions() {
        let img = solid_image(30, 20, Rgba([128, 128, 128, 255]));
        let out = GlitchEffect::PixelSort { threshold: 0.3 }.apply_image(img);
        assert_eq!(out.dimensions(), (30, 20));
    }

    #[test]
    fn block_shift_preserves_dimensions() {
        let img = solid_image(60, 45, Rgba([200, 100, 50, 255]));
        let out = GlitchEffect::BlockShift {
            shift_x: 10,
            shift_y: -5,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (60, 45));
    }

    #[test]
    fn block_shift_solid_image_unchanged() {
        let color = Rgba([200u8, 100, 50, 255]);
        let img = solid_image(60, 45, color);
        let out = GlitchEffect::BlockShift {
            shift_x: 10,
            shift_y: -5,
        }
        .apply_image(img);
        let rgba = out.to_rgba8();
        // Shifting a uniform image must not change any pixel.
        for p in rgba.pixels() {
            assert_eq!(*p, color);
        }
    }

    #[test]
    fn pixelate_block_size_one_is_identity() {
        let color = Rgba([42u8, 84, 126, 255]);
        let img = solid_image(20, 20, color);
        let out = GlitchEffect::Pixelate { block_size: 1 }.apply_image(img);
        // block_size == 1 is a no-op; every pixel is unchanged.
        let rgba = out.to_rgba8();
        for p in rgba.pixels() {
            assert_eq!(*p, color);
        }
    }
}
