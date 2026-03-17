use std::fmt;

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use serde::{Deserialize, Serialize};

use super::ParamDescriptor;

/// Glitch-style pixel effects: pixelate, row jitter, block shift, pixel sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GlitchEffect {
    /// Reduce effective resolution by grouping pixels into blocks.
    Pixelate { block_size: u32 },
    /// Randomly displace rows horizontally.
    RowJitter { magnitude: f32, seed: u32 },
    /// Shift rectangular blocks of pixels.
    BlockShift { shift_x: i32, shift_y: i32 },
    /// Sort pixels within rows by luminance.
    PixelSort { threshold: f32, reverse: bool },
    /// Overlay a Julia set fractal pattern blended with the source image.
    FractalJulia {
        scale: f32,
        cx: f32,
        cy: f32,
        max_iter: u32,
        blend: f32,
    },
    /// Create a low-poly look using Delaunay triangulation of random sample points.
    DelaunayTriangulation { num_points: u32, seed: u32 },
    GhostDisplace {
        copies: u32,
        offset_x: f32,
        offset_y: f32,
        hue_sweep: f32,
        opacity: f32,
    },
    /// Chromatic aberration: independently shift R, G, B channels by (x, y) offsets.
    RGBShift {
        x_r: i32,
        y_r: i32,
        x_g: i32,
        y_g: i32,
        x_b: i32,
        y_b: i32,
        wrap: bool,
    },
    /// Corrupt pixel data by applying bitwise operations (XOR, AND) or byte-swapping.
    DataBend { mode: u8, value: u8, seed: u32 },
    /// Displace rows or columns by a sine wave for rolling, wavy distortions.
    SineWarp {
        amplitude: f32,
        frequency: f32,
        phase: f32,
        axis: u8,
    },
    /// Simulate aggressive JPEG macroblocking by randomising and bleeding 8×8 blocks.
    JpegSmash {
        block_size: u32,
        strength: f32,
        bleed: bool,
    },
}

impl GlitchEffect {
    /// Apply this effect to an entire image, enabling full spatial context.
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            GlitchEffect::Pixelate { block_size } => pixelate(img, *block_size),
            GlitchEffect::RowJitter { magnitude, seed } => row_jitter(img, *magnitude, *seed),
            GlitchEffect::BlockShift { shift_x, shift_y } => block_shift(img, *shift_x, *shift_y),
            GlitchEffect::PixelSort { threshold, reverse } => pixel_sort(img, *threshold, *reverse),
            GlitchEffect::FractalJulia {
                scale,
                cx,
                cy,
                max_iter,
                blend,
            } => fractal_julia(img, *scale, *cx, *cy, *max_iter, *blend),
            GlitchEffect::DelaunayTriangulation { num_points, seed } => {
                delaunay_triangulation(img, *num_points, *seed)
            }
            GlitchEffect::GhostDisplace {
                copies,
                offset_x,
                offset_y,
                hue_sweep,
                opacity,
            } => ghost_displace(img, *copies, *offset_x, *offset_y, *hue_sweep, *opacity),
            GlitchEffect::RGBShift {
                x_r,
                y_r,
                x_g,
                y_g,
                x_b,
                y_b,
                wrap,
            } => rgb_shift(img, *x_r, *y_r, *x_g, *y_g, *x_b, *y_b, *wrap),
            GlitchEffect::DataBend { mode, value, seed } => data_bend(img, *mode, *value, *seed),
            GlitchEffect::SineWarp {
                amplitude,
                frequency,
                phase,
                axis,
            } => sine_warp(img, *amplitude, *frequency, *phase, *axis),
            GlitchEffect::JpegSmash {
                block_size,
                strength,
                bleed,
            } => jpeg_smash(img, *block_size, *strength, *bleed),
        }
    }

    /// Return descriptors for all editable numeric parameters.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            GlitchEffect::Pixelate { block_size } => vec![ParamDescriptor {
                name: "block_size",
                value: *block_size as f32,
                min: 1.0,
                max: 64.0,
            }],
            GlitchEffect::RowJitter { magnitude, seed } => vec![
                ParamDescriptor {
                    name: "magnitude",
                    value: *magnitude,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "seed",
                    value: *seed as f32,
                    min: 0.0,
                    max: 9999.0,
                },
            ],
            GlitchEffect::BlockShift { shift_x, shift_y } => vec![
                ParamDescriptor {
                    name: "shift_x",
                    value: *shift_x as f32,
                    min: -200.0,
                    max: 200.0,
                },
                ParamDescriptor {
                    name: "shift_y",
                    value: *shift_y as f32,
                    min: -200.0,
                    max: 200.0,
                },
            ],
            GlitchEffect::PixelSort { threshold, reverse } => vec![
                ParamDescriptor {
                    name: "threshold",
                    value: *threshold,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "reverse",
                    value: if *reverse { 1.0 } else { 0.0 },
                    min: 0.0,
                    max: 1.0,
                },
            ],
            GlitchEffect::FractalJulia {
                scale,
                cx,
                cy,
                max_iter,
                blend,
            } => vec![
                ParamDescriptor {
                    name: "scale",
                    value: *scale,
                    min: 0.1,
                    max: 5.0,
                },
                ParamDescriptor {
                    name: "cx",
                    value: *cx,
                    min: -2.0,
                    max: 2.0,
                },
                ParamDescriptor {
                    name: "cy",
                    value: *cy,
                    min: -2.0,
                    max: 2.0,
                },
                ParamDescriptor {
                    name: "max_iter",
                    value: *max_iter as f32,
                    min: 10.0,
                    max: 200.0,
                },
                ParamDescriptor {
                    name: "blend",
                    value: *blend,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            GlitchEffect::DelaunayTriangulation { num_points, seed } => vec![
                ParamDescriptor {
                    name: "num_points",
                    value: *num_points as f32,
                    min: 10.0,
                    max: 30000.0,
                },
                ParamDescriptor {
                    name: "seed",
                    value: *seed as f32,
                    min: 0.0,
                    max: 9999.0,
                },
            ],
            GlitchEffect::GhostDisplace {
                copies,
                offset_x,
                offset_y,
                hue_sweep,
                opacity,
            } => vec![
                ParamDescriptor {
                    name: "copies",
                    value: *copies as f32,
                    min: 1.0,
                    max: 12.0,
                },
                ParamDescriptor {
                    name: "offset_x",
                    value: *offset_x,
                    min: -100.0,
                    max: 100.0,
                },
                ParamDescriptor {
                    name: "offset_y",
                    value: *offset_y,
                    min: -100.0,
                    max: 100.0,
                },
                ParamDescriptor {
                    name: "hue_sweep",
                    value: *hue_sweep,
                    min: 0.0,
                    max: 360.0,
                },
                ParamDescriptor {
                    name: "opacity",
                    value: *opacity,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            GlitchEffect::RGBShift {
                x_r,
                y_r,
                x_g,
                y_g,
                x_b,
                y_b,
                wrap,
            } => vec![
                ParamDescriptor {
                    name: "x_r",
                    value: *x_r as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "y_r",
                    value: *y_r as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "x_g",
                    value: *x_g as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "y_g",
                    value: *y_g as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "x_b",
                    value: *x_b as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "y_b",
                    value: *y_b as f32,
                    min: -50.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "wrap",
                    value: if *wrap { 1.0 } else { 0.0 },
                    min: 0.0,
                    max: 1.0,
                },
            ],
            GlitchEffect::DataBend { mode, value, seed } => vec![
                ParamDescriptor {
                    name: "mode",
                    value: *mode as f32,
                    min: 0.0,
                    max: 2.0,
                },
                ParamDescriptor {
                    name: "value",
                    value: *value as f32,
                    min: 0.0,
                    max: 255.0,
                },
                ParamDescriptor {
                    name: "seed",
                    value: *seed as f32,
                    min: 0.0,
                    max: 9999.0,
                },
            ],
            GlitchEffect::SineWarp {
                amplitude,
                frequency,
                phase,
                axis,
            } => vec![
                ParamDescriptor {
                    name: "amplitude",
                    value: *amplitude,
                    min: 0.0,
                    max: 50.0,
                },
                ParamDescriptor {
                    name: "frequency",
                    value: *frequency,
                    min: 0.1,
                    max: 10.0,
                },
                ParamDescriptor {
                    name: "phase",
                    value: *phase,
                    min: 0.0,
                    max: 360.0,
                },
                ParamDescriptor {
                    name: "axis",
                    value: *axis as f32,
                    min: 0.0,
                    max: 1.0,
                },
            ],
            GlitchEffect::JpegSmash {
                block_size,
                strength,
                bleed,
            } => vec![
                ParamDescriptor {
                    name: "block_size",
                    value: *block_size as f32,
                    min: 4.0,
                    max: 32.0,
                },
                ParamDescriptor {
                    name: "strength",
                    value: *strength,
                    min: 0.0,
                    max: 1.0,
                },
                ParamDescriptor {
                    name: "bleed",
                    value: if *bleed { 1.0 } else { 0.0 },
                    min: 0.0,
                    max: 1.0,
                },
            ],
        }
    }

    /// Rebuild this variant with new parameter values (clamped to valid ranges).
    pub fn apply_params(&self, values: &[f32]) -> GlitchEffect {
        let get = |i: usize, fallback: f32| values.get(i).copied().unwrap_or(fallback);
        match self {
            GlitchEffect::Pixelate { block_size } => GlitchEffect::Pixelate {
                block_size: get(0, *block_size as f32) as u32,
            },
            GlitchEffect::RowJitter { magnitude, seed } => GlitchEffect::RowJitter {
                magnitude: get(0, *magnitude),
                seed: get(1, *seed as f32) as u32,
            },
            GlitchEffect::BlockShift { shift_x, shift_y } => GlitchEffect::BlockShift {
                shift_x: get(0, *shift_x as f32) as i32,
                shift_y: get(1, *shift_y as f32) as i32,
            },
            GlitchEffect::PixelSort { threshold, reverse } => GlitchEffect::PixelSort {
                threshold: get(0, *threshold),
                reverse: get(1, if *reverse { 1.0 } else { 0.0 }) >= 0.5,
            },
            GlitchEffect::FractalJulia {
                scale,
                cx,
                cy,
                max_iter,
                blend,
            } => GlitchEffect::FractalJulia {
                scale: get(0, *scale),
                cx: get(1, *cx),
                cy: get(2, *cy),
                max_iter: get(3, *max_iter as f32) as u32,
                blend: get(4, *blend),
            },
            GlitchEffect::DelaunayTriangulation { num_points, seed } => {
                GlitchEffect::DelaunayTriangulation {
                    num_points: get(0, *num_points as f32) as u32,
                    seed: get(1, *seed as f32) as u32,
                }
            }
            GlitchEffect::GhostDisplace {
                copies,
                offset_x,
                offset_y,
                hue_sweep,
                opacity,
            } => GlitchEffect::GhostDisplace {
                copies: get(0, *copies as f32) as u32,
                offset_x: get(1, *offset_x),
                offset_y: get(2, *offset_y),
                hue_sweep: get(3, *hue_sweep),
                opacity: get(4, *opacity),
            },
            GlitchEffect::RGBShift {
                x_r,
                y_r,
                x_g,
                y_g,
                x_b,
                y_b,
                wrap,
            } => GlitchEffect::RGBShift {
                x_r: get(0, *x_r as f32) as i32,
                y_r: get(1, *y_r as f32) as i32,
                x_g: get(2, *x_g as f32) as i32,
                y_g: get(3, *y_g as f32) as i32,
                x_b: get(4, *x_b as f32) as i32,
                y_b: get(5, *y_b as f32) as i32,
                wrap: get(6, if *wrap { 1.0 } else { 0.0 }) >= 0.5,
            },
            GlitchEffect::DataBend { mode, value, seed } => GlitchEffect::DataBend {
                mode: get(0, *mode as f32).clamp(0.0, 2.0) as u8,
                value: get(1, *value as f32) as u8,
                seed: get(2, *seed as f32) as u32,
            },
            GlitchEffect::SineWarp {
                amplitude,
                frequency,
                phase,
                axis,
            } => GlitchEffect::SineWarp {
                amplitude: get(0, *amplitude),
                frequency: get(1, *frequency),
                phase: get(2, *phase),
                axis: get(3, *axis as f32).clamp(0.0, 1.0) as u8,
            },
            GlitchEffect::JpegSmash {
                block_size,
                strength,
                bleed,
            } => GlitchEffect::JpegSmash {
                block_size: get(0, *block_size as f32).clamp(4.0, 32.0) as u32,
                strength: get(1, *strength),
                bleed: get(2, if *bleed { 1.0 } else { 0.0 }) >= 0.5,
            },
        }
    }

    /// Returns the variant name (e.g. `"Pixelate"`, `"RowJitter"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            GlitchEffect::Pixelate { .. } => "Pixelate",
            GlitchEffect::RowJitter { .. } => "RowJitter",
            GlitchEffect::BlockShift { .. } => "BlockShift",
            GlitchEffect::PixelSort { .. } => "PixelSort",
            GlitchEffect::FractalJulia { .. } => "FractalJulia",
            GlitchEffect::DelaunayTriangulation { .. } => "DelaunayTriangulation",
            GlitchEffect::GhostDisplace { .. } => "GhostDisplace",
            GlitchEffect::RGBShift { .. } => "RGBShift",
            GlitchEffect::DataBend { .. } => "DataBend",
            GlitchEffect::SineWarp { .. } => "SineWarp",
            GlitchEffect::JpegSmash { .. } => "JpegSmash",
        }
    }
}

impl fmt::Display for GlitchEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlitchEffect::Pixelate { block_size } => write!(f, "Pixelate {block_size}px"),
            GlitchEffect::RowJitter { magnitude, seed } => {
                write!(f, "RowJitter {magnitude:.2} s={seed}")
            }
            GlitchEffect::BlockShift { shift_x, shift_y } => {
                write!(f, "BlockShift ({shift_x},{shift_y})")
            }
            GlitchEffect::PixelSort { threshold, reverse } => {
                let dir = if *reverse { "desc" } else { "asc" };
                write!(f, "PixelSort {threshold:.2} {dir}")
            }
            GlitchEffect::FractalJulia {
                scale,
                cx,
                cy,
                max_iter,
                blend,
            } => {
                write!(
                    f,
                    "Julia s={scale:.1} c=({cx:.2},{cy:.2}) i={max_iter} b={blend:.2}"
                )
            }
            GlitchEffect::DelaunayTriangulation { num_points, seed } => {
                write!(f, "Delaunay pts={num_points} s={seed}")
            }
            GlitchEffect::GhostDisplace {
                copies,
                offset_x,
                offset_y,
                hue_sweep,
                opacity,
            } => {
                write!(
                    f,
                    "Ghost ×{copies} ({offset_x:.0},{offset_y:.0}) hue={hue_sweep:.0}° op={opacity:.2}"
                )
            }
            GlitchEffect::RGBShift { x_r, y_r, .. } => {
                write!(f, "RGBShift R({x_r},{y_r})")
            }
            GlitchEffect::DataBend { mode, value, seed } => {
                let m = match mode {
                    0 => "XOR",
                    1 => "AND",
                    _ => "Swap",
                };
                if *mode == 2 {
                    write!(f, "DataBend {m} s={seed}")
                } else {
                    write!(f, "DataBend {m} {value}")
                }
            }
            GlitchEffect::SineWarp {
                amplitude,
                frequency,
                axis,
                ..
            } => {
                let ax = if *axis == 0 { "rows" } else { "cols" };
                write!(f, "SineWarp A={amplitude:.1} F={frequency:.1} {ax}")
            }
            GlitchEffect::JpegSmash {
                block_size,
                strength,
                ..
            } => write!(f, "JpegSmash {block_size}px s={strength:.2}"),
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

fn row_jitter(img: DynamicImage, magnitude: f32, seed: u32) -> DynamicImage {
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
            let hash = (y as u32).wrapping_mul(2654435761).wrapping_add(seed)
                ^ (y as u32).wrapping_mul(2246822519).wrapping_add(seed);
            let n = ((hash as f32).sin() * 43_758.547).fract();
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

fn pixel_sort(img: DynamicImage, threshold: f32, reverse: bool) -> DynamicImage {
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
                    if reverse {
                        segment.sort_by(|a, b| luminance(b).partial_cmp(&luminance(a)).unwrap());
                    } else {
                        segment.sort_by(|a, b| luminance(a).partial_cmp(&luminance(b)).unwrap());
                    }
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

// ── Fractal Julia ────────────────────────────────────────────────────────────

fn fractal_julia(
    img: DynamicImage,
    scale: f32,
    cx: f32,
    cy: f32,
    max_iter: u32,
    blend: f32,
) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let mut out = ImageBuffer::new(w, h);
    let aspect = w as f32 / h as f32;
    let max_iter = max_iter.max(1);
    let blend = blend.clamp(0.0, 1.0);

    for y in 0..h {
        for x in 0..w {
            // Map pixel coordinates to complex plane centred at origin.
            let mut zr = (x as f32 / w as f32 - 0.5) * scale * aspect;
            let mut zi = (y as f32 / h as f32 - 0.5) * scale;

            let mut iter = 0u32;
            while zr * zr + zi * zi <= 4.0 && iter < max_iter {
                let tmp = zr * zr - zi * zi + cx;
                zi = 2.0 * zr * zi + cy;
                zr = tmp;
                iter += 1;
            }

            let t = iter as f32 / max_iter as f32;
            // Map iteration count to a colour via simple HSV-style ramp.
            let fr = ((t * 6.0).sin() * 0.5 + 0.5) * 255.0;
            let fg = ((t * 6.0 + 2.0).sin() * 0.5 + 0.5) * 255.0;
            let fb = ((t * 6.0 + 4.0).sin() * 0.5 + 0.5) * 255.0;

            let src = rgba.get_pixel(x, y);
            let r = (src[0] as f32 * (1.0 - blend) + fr * blend) as u8;
            let g = (src[1] as f32 * (1.0 - blend) + fg * blend) as u8;
            let b = (src[2] as f32 * (1.0 - blend) + fb * blend) as u8;
            out.put_pixel(x, y, Rgba([r, g, b, src[3]]));
        }
    }

    DynamicImage::ImageRgba8(out)
}

// ── Delaunay Triangulation ───────────────────────────────────────────────────

/// Simple LCG PRNG for deterministic point generation.
/// Returns a value in [0.0, 1.0).
fn lcg_next(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    // Shift right 33 gives a 31-bit value in [0, 2^31); divide by 2^31 to normalise.
    ((*state >> 33) as f32) / ((1u64 << 31) as f32)
}

fn delaunay_triangulation(img: DynamicImage, num_points: u32, seed: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let raw = rgba.as_raw();

    let num_points = num_points.max(4);
    let mut rng_state = seed as u64 ^ 0xDEAD_BEEF;

    // Generate random sample points + the 4 corners to cover the entire image.
    let mut points: Vec<(f32, f32)> = Vec::with_capacity(num_points as usize + 4);
    points.push((0.0, 0.0));
    points.push((w as f32 - 1.0, 0.0));
    points.push((0.0, h as f32 - 1.0));
    points.push((w as f32 - 1.0, h as f32 - 1.0));
    for _ in 0..num_points {
        let px = lcg_next(&mut rng_state) * (w as f32 - 1.0);
        let py = lcg_next(&mut rng_state) * (h as f32 - 1.0);
        points.push((px, py));
    }

    // Build Delaunay triangulation using Bowyer-Watson algorithm.
    let triangles = bowyer_watson(&points, w as f32, h as f32);

    // For each triangle, compute average colour from source image then fill.
    let mut out_raw = vec![0u8; raw.len()];
    let w_usize = w as usize;

    for tri in &triangles {
        let (ax, ay) = points[tri.0];
        let (bx, by) = points[tri.1];
        let (cx, cy) = points[tri.2];

        // Bounding box of the triangle.
        let min_x = ax.min(bx).min(cx).max(0.0) as u32;
        let max_x = (ax.max(bx).max(cx) as u32).min(w - 1);
        let min_y = ay.min(by).min(cy).max(0.0) as u32;
        let max_y = (ay.max(by).max(cy) as u32).min(h - 1);

        // Sample the centroid colour.
        let cent_x = ((ax + bx + cx) / 3.0).clamp(0.0, (w - 1) as f32) as u32;
        let cent_y = ((ay + by + cy) / 3.0).clamp(0.0, (h - 1) as f32) as u32;
        let ci = (cent_y as usize * w_usize + cent_x as usize) * 4;
        let col = [raw[ci], raw[ci + 1], raw[ci + 2], raw[ci + 3]];

        // Rasterise: fill all pixels inside the triangle.
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                if point_in_triangle(
                    (px as f32 + 0.5, py as f32 + 0.5),
                    (ax, ay),
                    (bx, by),
                    (cx, cy),
                ) {
                    let idx = (py as usize * w_usize + px as usize) * 4;
                    out_raw[idx..idx + 4].copy_from_slice(&col);
                }
            }
        }
    }

    let out = ImageBuffer::from_raw(w, h, out_raw).expect("buffer size mismatch");
    DynamicImage::ImageRgba8(out)
}

/// Test whether point `p` lies inside triangle `(a, b, c)` using barycentric
/// coordinates.
fn point_in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let v0x = c.0 - a.0;
    let v0y = c.1 - a.1;
    let v1x = b.0 - a.0;
    let v1y = b.1 - a.1;
    let v2x = p.0 - a.0;
    let v2y = p.1 - a.1;

    let dot00 = v0x * v0x + v0y * v0y;
    let dot01 = v0x * v1x + v0y * v1y;
    let dot02 = v0x * v2x + v0y * v2y;
    let dot11 = v1x * v1x + v1y * v1y;
    let dot12 = v1x * v2x + v1y * v2y;

    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < f32::EPSILON {
        return false; // degenerate triangle
    }
    let inv_denom = 1.0 / denom;
    // u, v are barycentric coordinates; point is inside when both ≥ 0 and u+v ≤ 1.
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    u >= 0.0 && v >= 0.0 && (u + v) <= 1.0
}

/// Bowyer-Watson incremental Delaunay triangulation.
fn bowyer_watson(points: &[(f32, f32)], w: f32, h: f32) -> Vec<(usize, usize, usize)> {
    // Large enough to fully contain all image-space points with safety margin.
    let margin = w.max(h) * 10.0;
    let sp0 = points.len();
    let sp1 = sp0 + 1;
    let sp2 = sp0 + 2;

    let mut all_points = points.to_vec();
    all_points.push((-margin, -margin));
    all_points.push((2.0 * margin + w, -margin));
    all_points.push((w / 2.0, 2.0 * margin + h));

    let mut triangulation: Vec<(usize, usize, usize)> = vec![(sp0, sp1, sp2)];

    for i in 0..points.len() {
        let (px, py) = all_points[i];
        let mut bad_triangles = Vec::new();

        for (ti, tri) in triangulation.iter().enumerate() {
            if in_circumcircle(px, py, &all_points, tri) {
                bad_triangles.push(ti);
            }
        }

        // Find the boundary polygon (edges that are not shared by two bad triangles).
        let mut polygon: Vec<(usize, usize)> = Vec::new();
        for &ti in &bad_triangles {
            let tri = triangulation[ti];
            let edges = [(tri.0, tri.1), (tri.1, tri.2), (tri.2, tri.0)];
            for edge in &edges {
                let shared = bad_triangles
                    .iter()
                    .any(|&oti| oti != ti && triangle_has_edge(triangulation[oti], *edge));
                if !shared {
                    polygon.push(*edge);
                }
            }
        }

        // Remove bad triangles (in reverse index order to preserve lower indices).
        bad_triangles.sort_unstable_by(|a, b| b.cmp(a));
        for ti in &bad_triangles {
            triangulation.swap_remove(*ti);
        }

        // Create new triangles from polygon edges to the new point.
        for edge in &polygon {
            triangulation.push((edge.0, edge.1, i));
        }
    }

    // Remove triangles that reference the super-triangle vertices.
    triangulation
        .retain(|tri| tri.0 < points.len() && tri.1 < points.len() && tri.2 < points.len());

    triangulation
}

fn in_circumcircle(px: f32, py: f32, points: &[(f32, f32)], tri: &(usize, usize, usize)) -> bool {
    let (ax, ay) = points[tri.0];
    let (bx, by) = points[tri.1];
    let (cx, cy) = points[tri.2];

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < f32::EPSILON {
        return false;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let r2 = (ax - ux) * (ax - ux) + (ay - uy) * (ay - uy);
    let dist2 = (px - ux) * (px - ux) + (py - uy) * (py - uy);

    dist2 <= r2
}

fn triangle_has_edge(tri: (usize, usize, usize), edge: (usize, usize)) -> bool {
    let edges = [(tri.0, tri.1), (tri.1, tri.2), (tri.2, tri.0)];
    edges
        .iter()
        .any(|e| (e.0 == edge.0 && e.1 == edge.1) || (e.0 == edge.1 && e.1 == edge.0))
}

fn ghost_displace(
    img: DynamicImage,
    copies: u32,
    offset_x: f32,
    offset_y: f32,
    hue_sweep: f32,
    opacity: f32,
) -> DynamicImage {
    if copies == 0 {
        return img;
    }

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let src = rgba.into_raw();
    let mut out = src.clone();
    let op = opacity.clamp(0.0, 1.0);

    for i in 0..copies {
        let t = if copies <= 1 {
            1.0
        } else {
            i as f32 / (copies - 1) as f32
        };
        let dx = (offset_x * t).round() as i32;
        let dy = (offset_y * t).round() as i32;
        let hue_shift = hue_sweep * t;

        for y in 0..h {
            for x in 0..w {
                let sx = x as i32 - dx;
                let sy = y as i32 - dy;
                if sx < 0 || sy < 0 || sx >= w as i32 || sy >= h as i32 {
                    continue;
                }

                let dst_idx = ((y * w + x) * 4) as usize;
                let src_idx = (((sy as u32) * w + sx as u32) * 4) as usize;

                let r = src[src_idx] as f32 / 255.0;
                let g = src[src_idx + 1] as f32 / 255.0;
                let b = src[src_idx + 2] as f32 / 255.0;
                let a = src[src_idx + 3] as f32 / 255.0;

                let (mut h_deg, s, l) = rgb_to_hsl(r, g, b);
                h_deg = (h_deg + hue_shift).rem_euclid(360.0);
                let (rr, gg, bb) = hsl_to_rgb(h_deg, s, l);

                let blend = op * a;
                out[dst_idx] = ((out[dst_idx] as f32) * (1.0 - blend) + rr * 255.0 * blend)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                out[dst_idx + 1] = ((out[dst_idx + 1] as f32) * (1.0 - blend) + gg * 255.0 * blend)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                out[dst_idx + 2] = ((out[dst_idx + 2] as f32) * (1.0 - blend) + bb * 255.0 * blend)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }

    let out_buf = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, out)
        .expect("ghost_displace must preserve buffer dimensions");
    DynamicImage::ImageRgba8(out_buf)
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let l = (max + min) * 0.5;

    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / d) + 2.0)
    } else {
        60.0 * (((r - g) / d) + 4.0)
    };

    (h, s, l)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
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

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= f32::EPSILON {
        return (l, l, l);
    }

    let h_norm = h / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    (
        hue_to_rgb(p, q, h_norm + 1.0 / 3.0),
        hue_to_rgb(p, q, h_norm),
        hue_to_rgb(p, q, h_norm - 1.0 / 3.0),
    )
}

// ── RGBShift ──────────────────────────────────────────────────────────────────

fn rgb_shift(
    img: DynamicImage,
    x_r: i32,
    y_r: i32,
    x_g: i32,
    y_g: i32,
    x_b: i32,
    y_b: i32,
    wrap: bool,
) -> DynamicImage {
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let wi = w as i32;
    let hi = h as i32;
    let src = rgba.into_raw();

    let sample = |px: i32, py: i32| -> (u8, u8, u8, u8) {
        let (sx, sy) = if wrap {
            (px.rem_euclid(wi) as u32, py.rem_euclid(hi) as u32)
        } else {
            (px.clamp(0, wi - 1) as u32, py.clamp(0, hi - 1) as u32)
        };
        let idx = ((sy * w + sx) * 4) as usize;
        (src[idx], src[idx + 1], src[idx + 2], src[idx + 3])
    };

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let xi = x as i32;
            let yi = y as i32;
            let (r, _, _, _) = sample(xi + x_r, yi + y_r);
            let (_, g, _, _) = sample(xi + x_g, yi + y_g);
            let (_, _, b, _) = sample(xi + x_b, yi + y_b);
            let (_, _, _, a) = sample(xi, yi);
            let idx = ((y * w + x) * 4) as usize;
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
            out[idx + 3] = a;
        }
    }

    let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, out)
        .expect("rgb_shift must preserve buffer dimensions");
    DynamicImage::ImageRgba8(buf)
}

// ── DataBend ──────────────────────────────────────────────────────────────────

fn data_bend(img: DynamicImage, mode: u8, value: u8, seed: u32) -> DynamicImage {
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let mut raw = rgba.into_raw();

    for y in 0..h {
        for x in 0..w {
            // Deterministic per-pixel hash.
            let hash = (x.wrapping_mul(2654435761).wrapping_add(seed))
                ^ (y.wrapping_mul(2246822519).wrapping_add(seed));
            // Affect ~30% of pixels.
            if (hash & 0xFF) >= 77 {
                continue;
            }
            let idx = ((y * w + x) * 4) as usize;
            match mode {
                0 => {
                    // XOR each channel
                    raw[idx] ^= value;
                    raw[idx + 1] ^= value;
                    raw[idx + 2] ^= value;
                }
                1 => {
                    // AND each channel
                    raw[idx] &= value;
                    raw[idx + 1] &= value;
                    raw[idx + 2] &= value;
                }
                _ => {
                    // Swap bytes with a deterministically-offset neighbour
                    let nx = ((x ^ (seed & 7)) % w) as usize;
                    let ny = ((y ^ ((seed >> 3) & 7)) % h) as usize;
                    let nidx = (ny * w as usize + nx) * 4;
                    for c in 0..3usize {
                        raw.swap(idx + c, nidx + c);
                    }
                }
            }
        }
    }

    let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, raw)
        .expect("data_bend must preserve buffer dimensions");
    DynamicImage::ImageRgba8(buf)
}

// ── SineWarp ──────────────────────────────────────────────────────────────────

fn sine_warp(img: DynamicImage, amplitude: f32, frequency: f32, phase: f32, axis: u8) -> DynamicImage {
    use std::f32::consts::TAU;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let src = rgba.into_raw();
    let mut out = vec![0u8; src.len()];
    let phase_rad = phase * TAU / 360.0;

    for y in 0..h {
        for x in 0..w {
            let (src_x, src_y) = if axis == 0 {
                // Displace rows horizontally
                let t = y as f32 / h as f32;
                let offset = amplitude * (TAU * frequency * t + phase_rad).sin();
                let sx = ((x as f32 + offset) as i32).rem_euclid(w as i32) as u32;
                (sx, y)
            } else {
                // Displace columns vertically
                let t = x as f32 / w as f32;
                let offset = amplitude * (TAU * frequency * t + phase_rad).sin();
                let sy = ((y as f32 + offset) as i32).rem_euclid(h as i32) as u32;
                (x, sy)
            };
            let dst_idx = ((y * w + x) * 4) as usize;
            let src_idx = ((src_y * w + src_x) * 4) as usize;
            out[dst_idx..dst_idx + 4].copy_from_slice(&src[src_idx..src_idx + 4]);
        }
    }

    let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, out)
        .expect("sine_warp must preserve buffer dimensions");
    DynamicImage::ImageRgba8(buf)
}

// ── JpegSmash ─────────────────────────────────────────────────────────────────

fn jpeg_smash(img: DynamicImage, block_size: u32, strength: f32, bleed: bool) -> DynamicImage {
    let block_size = block_size.max(1);
    let strength = strength.clamp(0.0, 1.0);
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let src = rgba.into_raw();
    let mut out = src.clone();

    let blocks_x = (w + block_size - 1) / block_size;
    let blocks_y = (h + block_size - 1) / block_size;

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let x0 = bx * block_size;
            let y0 = by * block_size;
            let x1 = (x0 + block_size).min(w);
            let y1 = (y0 + block_size).min(h);
            let cnt = ((x1 - x0) * (y1 - y0)) as u64;
            if cnt == 0 {
                continue;
            }

            // Compute average colour of the block.
            let (mut sr, mut sg, mut sb, mut sa) = (0u64, 0u64, 0u64, 0u64);
            for py in y0..y1 {
                for px in x0..x1 {
                    let idx = ((py * w + px) * 4) as usize;
                    sr += src[idx] as u64;
                    sg += src[idx + 1] as u64;
                    sb += src[idx + 2] as u64;
                    sa += src[idx + 3] as u64;
                }
            }
            let avg = [
                (sr / cnt) as u8,
                (sg / cnt) as u8,
                (sb / cnt) as u8,
                (sa / cnt) as u8,
            ];

            // Deterministic seed per block.
            let block_seed = bx.wrapping_mul(2654435761) ^ by.wrapping_mul(2246822519);
            let dice = (block_seed & 0xFF) as f32 / 255.0;

            // Decide whether to posterize (always) or bleed (if strength dice passes).
            let do_bleed = bleed && dice < strength;

            if do_bleed {
                // Use the average of a neighbouring block.
                let nbx = (bx + 1) % blocks_x;
                let nby = (by + 1) % blocks_y;
                let nx0 = nbx * block_size;
                let ny0 = nby * block_size;
                let nx1 = (nx0 + block_size).min(w);
                let ny1 = (ny0 + block_size).min(h);
                let ncnt = ((nx1 - nx0) * (ny1 - ny0)) as u64;
                if ncnt > 0 {
                    let (mut nr, mut ng, mut nb_val, mut na) = (0u64, 0u64, 0u64, 0u64);
                    for py in ny0..ny1 {
                        for px in nx0..nx1 {
                            let idx = ((py * w + px) * 4) as usize;
                            nr += src[idx] as u64;
                            ng += src[idx + 1] as u64;
                            nb_val += src[idx + 2] as u64;
                            na += src[idx + 3] as u64;
                        }
                    }
                    let navg = [
                        (nr / ncnt) as u8,
                        (ng / ncnt) as u8,
                        (nb_val / ncnt) as u8,
                        (na / ncnt) as u8,
                    ];
                    for py in y0..y1 {
                        for px in x0..x1 {
                            let idx = ((py * w + px) * 4) as usize;
                            out[idx..idx + 4].copy_from_slice(&navg);
                        }
                    }
                    continue;
                }
            }

            // Default: fill block with its own average colour.
            for py in y0..y1 {
                for px in x0..x1 {
                    let idx = ((py * w + px) * 4) as usize;
                    out[idx..idx + 4].copy_from_slice(&avg);
                }
            }
        }
    }

    let buf = ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, out)
        .expect("jpeg_smash must preserve buffer dimensions");
    DynamicImage::ImageRgba8(buf)
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
        let out = GlitchEffect::RowJitter {
            magnitude: 0.2,
            seed: 0,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (50, 40));
    }

    #[test]
    fn pixel_sort_preserves_dimensions() {
        let img = solid_image(30, 20, Rgba([128, 128, 128, 255]));
        let out = GlitchEffect::PixelSort {
            threshold: 0.3,
            reverse: false,
        }
        .apply_image(img);
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

    #[test]
    fn fractal_julia_preserves_dimensions() {
        let img = solid_image(40, 30, Rgba([100, 100, 100, 255]));
        let out = GlitchEffect::FractalJulia {
            scale: 2.0,
            cx: -0.7,
            cy: 0.27015,
            max_iter: 50,
            blend: 0.5,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (40, 30));
    }

    #[test]
    fn delaunay_triangulation_preserves_dimensions() {
        let img = solid_image(50, 40, Rgba([80, 120, 200, 255]));
        let out = GlitchEffect::DelaunayTriangulation {
            num_points: 50,
            seed: 42,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (50, 40));
    }

    #[test]
    fn ghost_displace_preserves_dimensions() {
        let img = solid_image(64, 48, Rgba([120, 80, 200, 255]));
        let out = GlitchEffect::GhostDisplace {
            copies: 5,
            offset_x: 8.0,
            offset_y: 4.0,
            hue_sweep: 60.0,
            opacity: 0.4,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (64, 48));
    }

    #[test]
    fn rgb_shift_preserves_dimensions() {
        let img = solid_image(40, 30, Rgba([100, 150, 200, 255]));
        let out = GlitchEffect::RGBShift {
            x_r: 3,
            y_r: 0,
            x_g: -2,
            y_g: 1,
            x_b: 0,
            y_b: -3,
            wrap: true,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (40, 30));
    }

    #[test]
    fn rgb_shift_zero_offsets_identity() {
        let color = Rgba([100u8, 150, 200, 255]);
        let img = solid_image(20, 20, color);
        let out = GlitchEffect::RGBShift {
            x_r: 0,
            y_r: 0,
            x_g: 0,
            y_g: 0,
            x_b: 0,
            y_b: 0,
            wrap: false,
        }
        .apply_image(img)
        .into_rgba8();
        for p in out.pixels() {
            assert_eq!(*p, color);
        }
    }

    #[test]
    fn data_bend_preserves_dimensions() {
        let img = solid_image(30, 30, Rgba([128, 64, 200, 255]));
        let out = GlitchEffect::DataBend {
            mode: 0,
            value: 42,
            seed: 0,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (30, 30));
    }

    #[test]
    fn data_bend_xor_zero_identity() {
        // XOR with 0 is identity for affected pixels.
        let color = Rgba([128u8, 64, 200, 255]);
        let img = solid_image(10, 10, color);
        let out = GlitchEffect::DataBend {
            mode: 0,
            value: 0,
            seed: 0,
        }
        .apply_image(img)
        .into_rgba8();
        for p in out.pixels() {
            assert_eq!(*p, color);
        }
    }

    #[test]
    fn sine_warp_preserves_dimensions() {
        let img = solid_image(50, 40, Rgba([80, 120, 180, 255]));
        let out = GlitchEffect::SineWarp {
            amplitude: 5.0,
            frequency: 2.0,
            phase: 0.0,
            axis: 0,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (50, 40));
    }

    #[test]
    fn sine_warp_zero_amplitude_identity() {
        let color = Rgba([80u8, 120, 180, 255]);
        let img = solid_image(30, 20, color);
        let out = GlitchEffect::SineWarp {
            amplitude: 0.0,
            frequency: 2.0,
            phase: 0.0,
            axis: 0,
        }
        .apply_image(img)
        .into_rgba8();
        for p in out.pixels() {
            assert_eq!(*p, color);
        }
    }

    #[test]
    fn jpeg_smash_preserves_dimensions() {
        let img = solid_image(64, 48, Rgba([200, 100, 50, 255]));
        let out = GlitchEffect::JpegSmash {
            block_size: 8,
            strength: 0.5,
            bleed: false,
        }
        .apply_image(img);
        assert_eq!(out.dimensions(), (64, 48));
    }
}
