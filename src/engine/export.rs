use anyhow::{Context, Result};
use std::path::PathBuf;

/// Export a processed image frame to disk as a lossless PNG.
///
/// The filename is auto-incremented if a file with the given name already exists,
/// preventing accidental overwrites.
pub fn export_image(image: &image::DynamicImage, output_path: PathBuf) -> Result<()> {
    let path = safe_path(output_path);
    image
        .save(&path)
        .with_context(|| format!("Failed to save image to {}", path.display()))?;
    Ok(())
}

/// Return a path that does not yet exist by appending `_N` before the extension.
fn safe_path(mut path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new(".")).to_path_buf();
    let mut n = 1u32;
    loop {
        path = parent.join(format!("{stem}_{n}{ext}"));
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}
