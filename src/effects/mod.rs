pub mod color;
pub mod composite;
pub mod crt;
pub mod glitch;

use std::fmt;

use image::{DynamicImage, GenericImageView, Rgba};
use serde::{Deserialize, Serialize};

use color::ColorEffect;
use composite::CompositeEffect;
use crt::CrtEffect;
use glitch::GlitchEffect;

/// Descriptor for a single editable parameter of an effect.
///
/// Boolean fields (e.g. `monochromatic`) are represented as `f32` using the
/// convention `0.0 = false`, `1.0 = true`; values ≥ 0.5 are treated as `true`
/// when converting back to `bool` in [`Effect::apply_params`].
#[derive(Debug, Clone)]
pub struct ParamDescriptor {
    pub name: &'static str,
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

/// Represents a single image-manipulation step in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Effect {
    Glitch(GlitchEffect),
    Color(ColorEffect),
    Crt(CrtEffect),
    Composite(CompositeEffect),
}

impl Effect {
    /// Apply this effect to an entire image buffer, enabling effects that need
    /// full spatial context (row jitter, pixel sort, scanlines with coordinates, …).
    pub fn apply_image(&self, img: DynamicImage) -> DynamicImage {
        match self {
            // Color effects – most are per-pixel; Dither needs full-image context.
            Effect::Color(e) => e.apply_image(img),
            // CRT effects that need coordinates.
            Effect::Crt(e) => e.apply_image(img),
            // Glitch effects need full-image context.
            Effect::Glitch(e) => e.apply_image(img),
            // Composite effects pass through here (blend needs secondary image).
            Effect::Composite(e) => e.apply_image(img),
        }
    }

    /// Return descriptors for all editable numeric parameters of this effect variant.
    ///
    /// Delegates to the variant-specific `param_descriptors()` method on each
    /// sub-effect type, keeping the `Effect` enum thin and making it easy to add
    /// new effects without touching this file.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            Effect::Color(e) => e.param_descriptors(),
            Effect::Glitch(e) => e.param_descriptors(),
            Effect::Crt(e) => e.param_descriptors(),
            Effect::Composite(e) => e.param_descriptors(),
        }
    }

    /// Rebuild this effect variant with a new set of parameter values (clamped to valid ranges).
    ///
    /// `values` must be in the same order as returned by [`Effect::param_descriptors`].
    /// Delegates to the variant-specific `apply_params()` method.
    pub fn apply_params(&self, values: &[f32]) -> Effect {
        match self {
            Effect::Color(e) => Effect::Color(e.apply_params(values)),
            Effect::Glitch(e) => Effect::Glitch(e.apply_params(values)),
            Effect::Crt(e) => Effect::Crt(e.apply_params(values)),
            Effect::Composite(e) => Effect::Composite(e.apply_params(values)),
        }
    }

    /// Returns the variant name (e.g. `"HueShift"`, `"Pixelate"`) for UI titles.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Effect::Color(e) => e.variant_name(),
            Effect::Glitch(e) => e.variant_name(),
            Effect::Crt(e) => e.variant_name(),
            Effect::Composite(e) => e.variant_name(),
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Color(e) => e.fmt(f),
            Effect::Glitch(e) => e.fmt(f),
            Effect::Crt(e) => e.fmt(f),
            Effect::Composite(e) => e.fmt(f),
        }
    }
}

/// A single pipeline step with an optional enable/disable flag.
///
/// When `enabled` is `false` the wrapped [`Effect`] is skipped during
/// [`Pipeline::apply_image`] without being removed from the pipeline,
/// allowing quick A/B comparisons.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnabledEffect {
    /// Whether this effect participates in the pipeline.
    pub enabled: bool,
    /// The wrapped image-manipulation effect.
    pub effect: Effect,
}

impl EnabledEffect {
    /// Create a new enabled effect wrapping `effect`.
    pub fn new(effect: Effect) -> Self {
        Self {
            enabled: true,
            effect,
        }
    }
}

/// An ordered sequence of [`EnabledEffect`]s applied to the image.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pipeline {
    pub effects: Vec<EnabledEffect>,
}

impl Pipeline {
    /// Apply all **enabled** effects in the pipeline to a full image buffer.
    pub fn apply_image(&self, mut img: DynamicImage) -> DynamicImage {
        for (i, ee) in self.effects.iter().enumerate() {
            if ee.enabled {
                log::debug!("Pipeline step {i}: applying {:?}", ee.effect);
                let step_start = std::time::Instant::now();
                img = ee.effect.apply_image(img);
                log::debug!("Pipeline step {i}: completed in {:?}", step_start.elapsed());
            } else {
                log::debug!("Pipeline step {i}: skipped (disabled)");
            }
        }
        img
    }
}

// ── Helper ───────────────────────────────────────────────────────────────────

/// Apply a pixel-level function sequentially across all pixels.
///
/// Operates directly on the raw RGBA byte slice via `chunks_mut(4)`,
/// keeping allocations minimal and enabling LLVM to auto-vectorise the
/// inner loop via SIMD when compiling with optimisations (`--release` or
/// equivalent `-O2`/`-O3` flags).
pub fn apply_per_pixel<F>(img: DynamicImage, f: F) -> DynamicImage
where
    F: Fn(Rgba<u8>, u32, u32) -> Rgba<u8>,
{
    let (width, _height) = img.dimensions();
    let mut rgba = img.into_rgba8();
    rgba.chunks_mut(4).enumerate().for_each(|(i, chunk)| {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        let result = f(Rgba([chunk[0], chunk[1], chunk[2], chunk[3]]), x, y);
        chunk.copy_from_slice(&result.0);
    });
    DynamicImage::ImageRgba8(rgba)
}
