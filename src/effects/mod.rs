pub mod color;
pub mod composite;
pub mod crt;
pub mod glitch;

use image::{DynamicImage, ImageBuffer, Rgba};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use color::ColorEffect;
use composite::CompositeEffect;
use crt::CrtEffect;
use glitch::GlitchEffect;

/// Represents a single image-manipulation step in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    Glitch(GlitchEffect),
    Color(ColorEffect),
    Crt(CrtEffect),
    Composite(CompositeEffect),
}

impl Effect {
    /// Apply this effect to a single pixel (stateless, no coordinate context).
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            Effect::Color(e) => e.apply_pixel(pixel),
            Effect::Glitch(e) => e.apply_pixel(pixel),
            Effect::Crt(e) => e.apply_pixel(pixel),
            Effect::Composite(e) => e.apply_pixel(pixel),
        }
    }

    /// Apply this effect to an entire image buffer, enabling effects that need
    /// full spatial context (row jitter, pixel sort, scanlines with coordinates, …).
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            // Per-pixel colour effects are embarrassingly parallel.
            Effect::Color(e) => apply_per_pixel_parallel(img, |p, _, _| e.apply_pixel(p)),
            // CRT effects that need coordinates.
            Effect::Crt(e) => {
                let (w, h) = (img.width(), img.height());
                apply_per_pixel_parallel(img, move |p, x, y| {
                    e.apply_pixel_with_coords(p, x, y, w, h)
                })
            }
            // Glitch effects need full-image context.
            Effect::Glitch(e) => e.apply_image(img),
            // Composite effects pass through here (blend needs secondary image).
            Effect::Composite(e) => e.apply_image(img),
        }
    }
}

/// An ordered sequence of [`Effect`]s applied to the image.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pipeline {
    pub effects: Vec<Effect>,
}

impl Pipeline {
    /// Apply all effects in the pipeline to a single pixel.
    pub fn apply_pixel(&self, mut pixel: Rgba<u8>) -> Rgba<u8> {
        for effect in &self.effects {
            pixel = effect.apply_pixel(pixel);
        }
        pixel
    }

    /// Apply all effects in the pipeline to a full image buffer.
    pub fn apply_image(&self, mut img: DynamicImage) -> DynamicImage {
        for effect in &self.effects {
            img = effect.apply_image(img);
        }
        img
    }
}

// ── Helper ───────────────────────────────────────────────────────────────────

/// Apply a pixel-level function in parallel across all pixels, using rayon.
pub fn apply_per_pixel_parallel<F>(img: DynamicImage, f: F) -> DynamicImage
where
    F: Fn(Rgba<u8>, u32, u32) -> Rgba<u8> + Sync + Send,
{
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Collect (x, y, pixel) tuples.
    let pixels: Vec<(u32, u32, Rgba<u8>)> = rgba
        .enumerate_pixels()
        .map(|(x, y, p)| (x, y, *p))
        .collect();

    let processed: Vec<(u32, u32, Rgba<u8>)> = pixels
        .par_iter()
        .map(|(x, y, p)| (*x, *y, f(*p, *x, *y)))
        .collect();

    let mut out = ImageBuffer::new(width, height);
    for (x, y, p) in processed {
        out.put_pixel(x, y, p);
    }
    DynamicImage::ImageRgba8(out)
}
