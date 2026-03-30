use image::{DynamicImage, Rgba, RgbaImage};
use std::time::Instant;

// We will copy the edge_glow logic here for benchmark comparison
fn edge_glow_original(img: DynamicImage) -> DynamicImage {
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

    // just return something to avoid compiling away
    out.put_pixel(0, 0, Rgba([gray[0] as u8, 0, 0, 0]));
    DynamicImage::ImageRgba8(out)
}

fn edge_glow_optimized(img: DynamicImage) -> DynamicImage {
    let src = img.into_rgba8();
    let (w, h) = src.dimensions();
    let mut out = src.clone();
    if w == 0 || h == 0 {
        return DynamicImage::ImageRgba8(out);
    }

    let gray: Vec<f32> = src
        .pixels()
        .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0)
        .collect();

    // just return something to avoid compiling away
    out.put_pixel(0, 0, Rgba([gray[0] as u8, 0, 0, 0]));
    DynamicImage::ImageRgba8(out)
}

fn main() {
    let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        2000,
        2000,
        Rgba([100, 110, 120, 255]),
    ));

    // warmup
    for _ in 0..5 {
        edge_glow_original(img.clone());
        edge_glow_optimized(img.clone());
    }

    let iters = 20;

    let start = Instant::now();
    for _ in 0..iters {
        edge_glow_original(img.clone());
    }
    let orig_duration = start.elapsed();

    let start = Instant::now();
    for _ in 0..iters {
        edge_glow_optimized(img.clone());
    }
    let opt_duration = start.elapsed();

    println!("Original: {:?}", orig_duration / iters);
    println!("Optimized: {:?}", opt_duration / iters);
}
