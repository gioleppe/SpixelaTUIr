pub mod color;
pub mod composite;
pub mod crt;
pub mod glitch;

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
            // Per-pixel colour effects – the tight inner loop is auto-vectorised by LLVM.
            Effect::Color(e) => apply_per_pixel(img, |p, _, _| e.apply_pixel(p)),
            // CRT effects that need coordinates.
            Effect::Crt(e) => {
                let (w, h) = (img.width(), img.height());
                apply_per_pixel(img, move |p, x, y| e.apply_pixel_with_coords(p, x, y, w, h))
            }
            // Glitch effects need full-image context.
            Effect::Glitch(e) => e.apply_image(img),
            // Composite effects pass through here (blend needs secondary image).
            Effect::Composite(e) => e.apply_image(img),
        }
    }

    /// Return descriptors for all editable numeric parameters of this effect variant.
    pub fn param_descriptors(&self) -> Vec<ParamDescriptor> {
        match self {
            Effect::Color(e) => match e {
                ColorEffect::Invert => vec![],
                ColorEffect::GradientMap { preset_idx, stops } => {
                    let mut params = vec![ParamDescriptor {
                        name: "preset",
                        value: *preset_idx as f32,
                        min: 0.0,
                        max: (color::GRADIENT_PRESETS.len() - 1) as f32,
                    }];

                    // If it's the "Custom" preset (last one), allow editing colors.
                    if *preset_idx == color::GRADIENT_PRESETS.len() - 1 && stops.len() >= 2 {
                        params.push(ParamDescriptor {
                            name: "r1",
                            value: stops[0].1[0] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                        params.push(ParamDescriptor {
                            name: "g1",
                            value: stops[0].1[1] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                        params.push(ParamDescriptor {
                            name: "b1",
                            value: stops[0].1[2] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                        params.push(ParamDescriptor {
                            name: "r2",
                            value: stops[1].1[0] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                        params.push(ParamDescriptor {
                            name: "g2",
                            value: stops[1].1[1] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                        params.push(ParamDescriptor {
                            name: "b2",
                            value: stops[1].1[2] as f32,
                            min: 0.0,
                            max: 255.0,
                        });
                    }
                    params
                }
                ColorEffect::HueShift { degrees } => vec![ParamDescriptor {
                    name: "degrees",
                    value: *degrees,
                    min: 0.0,
                    max: 360.0,
                }],
                ColorEffect::Contrast { factor } => vec![ParamDescriptor {
                    name: "factor",
                    value: *factor,
                    min: 0.1,
                    max: 3.0,
                }],
                ColorEffect::Saturation { factor } => vec![ParamDescriptor {
                    name: "factor",
                    value: *factor,
                    min: 0.0,
                    max: 2.0,
                }],
                ColorEffect::ColorQuantization { levels } => vec![ParamDescriptor {
                    name: "levels",
                    value: *levels as f32,
                    min: 2.0,
                    max: 16.0,
                }],
            },
            Effect::Glitch(e) => match e {
                GlitchEffect::Pixelate { block_size } => vec![ParamDescriptor {
                    name: "block_size",
                    value: *block_size as f32,
                    min: 1.0,
                    max: 64.0,
                }],
                GlitchEffect::RowJitter { magnitude } => vec![ParamDescriptor {
                    name: "magnitude",
                    value: *magnitude,
                    min: 0.0,
                    max: 1.0,
                }],
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
                GlitchEffect::PixelSort { threshold } => vec![ParamDescriptor {
                    name: "threshold",
                    value: *threshold,
                    min: 0.0,
                    max: 1.0,
                }],
            },
            Effect::Crt(e) => match e {
                CrtEffect::Scanlines { spacing, opacity } => vec![
                    ParamDescriptor {
                        name: "spacing",
                        value: *spacing as f32,
                        min: 1.0,
                        max: 20.0,
                    },
                    ParamDescriptor {
                        name: "opacity",
                        value: *opacity,
                        min: 0.0,
                        max: 1.0,
                    },
                ],
                CrtEffect::Curvature { strength } => vec![ParamDescriptor {
                    name: "strength",
                    value: *strength,
                    min: 0.0,
                    max: 1.0,
                }],
                CrtEffect::PhosphorGlow { radius, intensity } => vec![
                    ParamDescriptor {
                        name: "radius",
                        value: *radius as f32,
                        min: 1.0,
                        max: 20.0,
                    },
                    ParamDescriptor {
                        name: "intensity",
                        value: *intensity,
                        min: 0.0,
                        max: 1.0,
                    },
                ],
                CrtEffect::Noise {
                    intensity,
                    monochromatic,
                } => vec![
                    ParamDescriptor {
                        name: "intensity",
                        value: *intensity,
                        min: 0.0,
                        max: 1.0,
                    },
                    ParamDescriptor {
                        name: "monochromatic",
                        value: if *monochromatic { 1.0 } else { 0.0 },
                        min: 0.0,
                        max: 1.0,
                    },
                ],
                CrtEffect::Vignette { radius, softness } => vec![
                    ParamDescriptor {
                        name: "radius",
                        value: *radius,
                        min: 0.0,
                        max: 2.0,
                    },
                    ParamDescriptor {
                        name: "softness",
                        value: *softness,
                        min: 0.0,
                        max: 2.0,
                    },
                ],
            },
            Effect::Composite(e) => match e {
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
            },
        }
    }

    /// Rebuild this effect variant with a new set of parameter values (clamped to valid ranges).
    ///
    /// `values` must be in the same order as returned by [`Effect::param_descriptors`].
    pub fn apply_params(&self, values: &[f32]) -> Effect {
        let get = |i: usize, fallback: f32| values.get(i).copied().unwrap_or(fallback);
        match self {
            Effect::Color(e) => Effect::Color(match e {
                ColorEffect::Invert => ColorEffect::Invert,
                ColorEffect::GradientMap { preset_idx, stops } => {
                    let new_preset_idx = get(0, *preset_idx as f32) as usize;
                    if new_preset_idx != *preset_idx {
                        // Preset changed, load the new preset's default stops.
                        ColorEffect::GradientMap {
                            preset_idx: new_preset_idx,
                            stops: color::GRADIENT_PRESETS[new_preset_idx].1.to_vec(),
                        }
                    } else if new_preset_idx == color::GRADIENT_PRESETS.len() - 1 {
                        // "Custom" preset, update colors from params if provided.
                        let mut new_stops = stops.clone();
                        if new_stops.len() >= 2 && values.len() >= 7 {
                            new_stops[0].1 = [get(1, 0.0) as u8, get(2, 0.0) as u8, get(3, 0.0) as u8];
                            new_stops[1].1 = [get(4, 0.0) as u8, get(5, 0.0) as u8, get(6, 0.0) as u8];
                        }
                        ColorEffect::GradientMap {
                            preset_idx: new_preset_idx,
                            stops: new_stops,
                        }
                    } else {
                        // Preset unchanged (and not custom), keep current stops.
                        ColorEffect::GradientMap {
                            preset_idx: *preset_idx,
                            stops: stops.clone(),
                        }
                    }
                }
                ColorEffect::HueShift { degrees } => ColorEffect::HueShift {
                    degrees: get(0, *degrees),
                },
                ColorEffect::Contrast { factor } => ColorEffect::Contrast {
                    factor: get(0, *factor),
                },
                ColorEffect::Saturation { factor } => ColorEffect::Saturation {
                    factor: get(0, *factor),
                },
                ColorEffect::ColorQuantization { levels } => ColorEffect::ColorQuantization {
                    levels: get(0, *levels as f32) as u8,
                },
            }),
            Effect::Glitch(e) => Effect::Glitch(match e {
                GlitchEffect::Pixelate { block_size } => GlitchEffect::Pixelate {
                    block_size: get(0, *block_size as f32) as u32,
                },
                GlitchEffect::RowJitter { magnitude } => GlitchEffect::RowJitter {
                    magnitude: get(0, *magnitude),
                },
                GlitchEffect::BlockShift { shift_x, shift_y } => GlitchEffect::BlockShift {
                    shift_x: get(0, *shift_x as f32) as i32,
                    shift_y: get(1, *shift_y as f32) as i32,
                },
                GlitchEffect::PixelSort { threshold } => GlitchEffect::PixelSort {
                    threshold: get(0, *threshold),
                },
            }),
            Effect::Crt(e) => Effect::Crt(match e {
                CrtEffect::Scanlines { spacing, opacity } => CrtEffect::Scanlines {
                    spacing: get(0, *spacing as f32) as u32,
                    opacity: get(1, *opacity),
                },
                CrtEffect::Curvature { strength } => CrtEffect::Curvature {
                    strength: get(0, *strength),
                },
                CrtEffect::PhosphorGlow { radius, intensity } => CrtEffect::PhosphorGlow {
                    radius: get(0, *radius as f32) as u32,
                    intensity: get(1, *intensity),
                },
                CrtEffect::Noise {
                    intensity,
                    monochromatic,
                } => CrtEffect::Noise {
                    intensity: get(0, *intensity),
                    // Boolean encoded as float: value >= 0.5 → true (see ParamDescriptor docs).
                    monochromatic: get(1, if *monochromatic { 1.0 } else { 0.0 }) >= 0.5,
                },
                CrtEffect::Vignette { radius, softness } => CrtEffect::Vignette {
                    radius: get(0, *radius),
                    softness: get(1, *softness),
                },
            }),
            Effect::Composite(e) => Effect::Composite(match e {
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
            }),
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
