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
    /// Apply per-pixel transformation (simplified stub for pipeline composition).
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        pixel
    }

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
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let mut out = ImageBuffer::new(w, h);

    let bw = block_size;
    let bh = block_size;
    let mut by = 0u32;
    while by < h {
        let block_h = bh.min(h - by);
        let mut bx = 0u32;
        while bx < w {
            let block_w = bw.min(w - bx);
            // Average colour within this block.
            let (mut sr, mut sg, mut sb, mut sa, mut cnt) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for dy in 0..block_h {
                for dx in 0..block_w {
                    let p = rgba.get_pixel(bx + dx, by + dy);
                    sr += p[0] as u64;
                    sg += p[1] as u64;
                    sb += p[2] as u64;
                    sa += p[3] as u64;
                    cnt += 1;
                }
            }
            let avg = Rgba([
                (sr / cnt) as u8,
                (sg / cnt) as u8,
                (sb / cnt) as u8,
                (sa / cnt) as u8,
            ]);
            for dy in 0..block_h {
                for dx in 0..block_w {
                    out.put_pixel(bx + dx, by + dy, avg);
                }
            }
            bx += bw;
        }
        by += bh;
    }
    DynamicImage::ImageRgba8(out)
}

// ── Row Jitter ────────────────────────────────────────────────────────────────

fn row_jitter(img: DynamicImage, magnitude: f32) -> DynamicImage {
    use rayon::prelude::*;

    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let max_shift = (w as f32 * magnitude.abs()) as i32;

    // Collect all rows as independent slices.
    let rows: Vec<Vec<image::Rgba<u8>>> = (0..h)
        .map(|y| (0..w).map(|x| *rgba.get_pixel(x, y)).collect())
        .collect();

    // Shift each row independently and in parallel.
    let shifted_rows: Vec<Vec<image::Rgba<u8>>> = rows
        .par_iter()
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
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let mut out = ImageBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let src_x = ((x as i32 + shift_x).rem_euclid(w as i32)) as u32;
            let src_y = ((y as i32 + shift_y).rem_euclid(h as i32)) as u32;
            out.put_pixel(x, y, *rgba.get_pixel(src_x, src_y));
        }
    }
    DynamicImage::ImageRgba8(out)
}

// ── Pixel Sort ────────────────────────────────────────────────────────────────

fn luminance(p: &Rgba<u8>) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn pixel_sort(img: DynamicImage, threshold: f32) -> DynamicImage {
    use rayon::prelude::*;

    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let thresh = threshold * 255.0;

    // Process each row independently and in parallel.
    let sorted_rows: Vec<Vec<Rgba<u8>>> = (0..h)
        .map(|y| (0..w).map(|x| *rgba.get_pixel(x, y)).collect::<Vec<_>>())
        .collect::<Vec<_>>()
        .into_par_iter()
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
    fn pixelate_solid_image_unchanged() {
        let color = Rgba([100u8, 150, 200, 255]);
        let img = solid_image(40, 40, color);
        let out = GlitchEffect::Pixelate { block_size: 8 }.apply_image(img);
        let rgba = out.to_rgba8();
        // Every pixel should still be the same colour.
        for p in rgba.pixels() {
            assert_eq!(*p, color);
        }
    }
}
