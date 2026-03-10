pub mod color;
pub mod composite;
pub mod crt;
pub mod glitch;

use image::Rgba;
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
    /// Apply this effect to a single pixel.
    ///
    /// Per-pixel application is used when running the pipeline through rayon.
    /// Effects that require global context (e.g. row/block operations) should
    /// operate on the full image buffer instead.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            Effect::Color(e) => e.apply_pixel(pixel),
            Effect::Glitch(e) => e.apply_pixel(pixel),
            Effect::Crt(e) => e.apply_pixel(pixel),
            Effect::Composite(e) => e.apply_pixel(pixel),
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
}
