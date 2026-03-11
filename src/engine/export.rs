use anyhow::{Context, Result};
use std::path::PathBuf;

/// Supported output formats for image export.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Png,
    Jpeg { quality: u8 },
    WebP,
    Bmp,
}

impl ExportFormat {
    /// File extension (without leading dot).
    pub fn extension(&self) -> &str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Jpeg { .. } => "jpg",
            ExportFormat::WebP => "webp",
            ExportFormat::Bmp => "bmp",
        }
    }

    /// Short display name shown in the UI.
    pub fn display_name(&self) -> &str {
        match self {
            ExportFormat::Png => "PNG",
            ExportFormat::Jpeg { .. } => "JPEG",
            ExportFormat::WebP => "WebP",
            ExportFormat::Bmp => "BMP",
        }
    }
}

/// All formats available in the export dialog, in display order.
pub const EXPORT_FORMATS: &[ExportFormat] = &[
    ExportFormat::Png,
    ExportFormat::Jpeg { quality: 90 },
    ExportFormat::WebP,
    ExportFormat::Bmp,
];

/// Export a processed image frame to disk in the requested format.
///
/// The filename is auto-incremented if a file with the given name already exists,
/// preventing accidental overwrites.
pub fn export_image(
    image: &image::DynamicImage,
    output_path: PathBuf,
    format: &ExportFormat,
) -> Result<PathBuf> {
    let path = safe_path(output_path);
    match format {
        ExportFormat::Jpeg { quality } => {
            let file = std::fs::File::create(&path)
                .with_context(|| format!("Failed to create {}", path.display()))?;
            let mut writer = std::io::BufWriter::new(file);
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, *quality);
            encoder
                .encode_image(image)
                .with_context(|| format!("Failed to encode JPEG to {}", path.display()))?;
        }
        ExportFormat::Png => {
            image
                .save_with_format(&path, image::ImageFormat::Png)
                .with_context(|| format!("Failed to save PNG to {}", path.display()))?;
        }
        ExportFormat::WebP => {
            image
                .save_with_format(&path, image::ImageFormat::WebP)
                .with_context(|| format!("Failed to save WebP to {}", path.display()))?;
        }
        ExportFormat::Bmp => {
            image
                .save_with_format(&path, image::ImageFormat::Bmp)
                .with_context(|| format!("Failed to save BMP to {}", path.display()))?;
        }
    }
    Ok(path)
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
    let parent = path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    let mut n = 1u32;
    loop {
        path = parent.join(format!("{stem}_{n}{ext}"));
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}
