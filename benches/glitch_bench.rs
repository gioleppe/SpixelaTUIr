use criterion::{criterion_group, criterion_main, Criterion};
use image::{DynamicImage, ImageBuffer, Rgba};

#[path = "../src/effects/mod.rs"]
pub mod effects;

use effects::glitch::GlitchEffect;

fn solid_image(w: u32, h: u32, color: Rgba<u8>) -> DynamicImage {
    let buf = ImageBuffer::from_pixel(w, h, color);
    DynamicImage::ImageRgba8(buf)
}

fn bench_fractal_julia(c: &mut Criterion) {
    let img = solid_image(200, 200, Rgba([100, 100, 100, 255]));
    let effect = GlitchEffect::FractalJulia {
        scale: 2.0,
        cx: -0.7,
        cy: 0.27015,
        max_iter: 50,
        blend: 0.5,
    };
    c.bench_function("fractal_julia_200x200_iter50", |b| {
        b.iter(|| effect.apply_image(std::hint::black_box(img.clone())))
    });
}

criterion_group!(benches, bench_fractal_julia);
criterion_main!(benches);
