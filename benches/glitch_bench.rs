use criterion::{criterion_group, criterion_main, Criterion};
use image::{DynamicImage, GenericImageView, RgbaImage};
use std::hint::black_box;

pub fn row_jitter_baseline(img: &DynamicImage, magnitude: f32, _seed: u32) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let _max_shift = (w as f32 * magnitude.abs()) as i32;

    let rows: Vec<Vec<image::Rgba<u8>>> = (0..h)
        .map(|y| (0..w).map(|x| *rgba.get_pixel(x, y)).collect())
        .collect();
    black_box(rows);
}

pub fn row_jitter_optimized(img: &DynamicImage, magnitude: f32, _seed: u32) {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let _max_shift = (w as f32 * magnitude.abs()) as i32;

    let rows: Vec<Vec<image::Rgba<u8>>> = rgba.rows().map(|row| row.copied().collect()).collect();
    black_box(rows);
}

fn criterion_benchmark(c: &mut Criterion) {
    let img = DynamicImage::ImageRgba8(RgbaImage::new(1920, 1080));

    c.bench_function("row_jitter_baseline", |b| b.iter(|| row_jitter_baseline(&img, 0.1, 42)));
    c.bench_function("row_jitter_optimized", |b| b.iter(|| row_jitter_optimized(&img, 0.1, 42)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
