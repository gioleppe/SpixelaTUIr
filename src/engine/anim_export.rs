//! Animation export — encodes a sequence of frames to GIF or animated WebP.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame};

/// Export an animation to a file.
///
/// `frames` is a slice of `(image, duration_ms)` pairs.  
/// `format_index`: 0 = GIF, 1 = WebP (falls back to GIF if animated WebP
/// is not available in the current build of the `image` crate).
pub fn export_animation(
    frames: &[(image::DynamicImage, u32)],
    output_path: PathBuf,
    format_index: usize,
    loop_anim: bool,
) -> Result<PathBuf> {
    match format_index {
        1 => export_webp(frames, output_path, loop_anim),
        _ => export_gif(frames, output_path, loop_anim),
    }
}

// ── GIF ──────────────────────────────────────────────────────────────────────

fn export_gif(
    frames: &[(image::DynamicImage, u32)],
    output_path: PathBuf,
    loop_anim: bool,
) -> Result<PathBuf> {
    let path = super::export::safe_path(output_path);
    let file = std::fs::File::create(&path)
        .with_context(|| format!("Cannot create {}", path.display()))?;
    let writer = std::io::BufWriter::new(file);

    let mut encoder = GifEncoder::new_with_speed(writer, 10);
    encoder
        .set_repeat(if loop_anim {
            Repeat::Infinite
        } else {
            Repeat::Finite(0)
        })
        .with_context(|| "Failed to set GIF repeat")?;

    for (img, duration_ms) in frames {
        let rgba = img.to_rgba8();
        let delay = Delay::from_saturating_duration(Duration::from_millis(*duration_ms as u64));
        let frame = Frame::from_parts(rgba, 0, 0, delay);
        encoder
            .encode_frame(frame)
            .with_context(|| "Failed to encode GIF frame")?;
    }

    Ok(path)
}

// ── WebP ─────────────────────────────────────────────────────────────────────

/// Animated WebP is not yet supported by the `image` crate.  This falls back
/// to GIF export (changing the file extension to `.gif`) — the UI labels
/// this option "WebP" for forward compatibility, but users should prefer GIF
/// until a dedicated animated-WebP crate is integrated.
fn export_webp(
    frames: &[(image::DynamicImage, u32)],
    mut output_path: PathBuf,
    loop_anim: bool,
) -> Result<PathBuf> {
    // Override the extension to .gif so the file is actually a valid GIF.
    output_path.set_extension("gif");
    log::warn!("Animated WebP not yet supported – exporting as GIF instead");
    export_gif(frames, output_path, loop_anim)
}
