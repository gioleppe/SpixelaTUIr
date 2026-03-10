use image::Rgba;
use serde::{Deserialize, Serialize};

/// CRT-style post-processing effects: scanlines, curvature, phosphor glow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrtEffect {
    /// Draw horizontal scanlines every N rows.
    Scanlines { spacing: u32, opacity: f32 },
    /// Apply barrel-distortion curvature to simulate a curved CRT screen.
    Curvature { strength: f32 },
    /// Blur and add a coloured halo to bright regions.
    PhosphorGlow { radius: u32, intensity: f32 },
}

impl CrtEffect {
    /// Apply per-pixel CRT transformation.
    pub fn apply_pixel(&self, pixel: Rgba<u8>) -> Rgba<u8> {
        match self {
            CrtEffect::Scanlines { .. } => pixel,
            CrtEffect::Curvature { .. } => pixel,
            CrtEffect::PhosphorGlow { .. } => pixel,
        }
    }
}
