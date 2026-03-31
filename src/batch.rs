//! Headless batch-processing mode.
//!
//! Invoked when the user passes `--batch <glob> --pipeline <file> --outdir <dir>`
//! on the command line.  The TUI is never started; images matching the glob
//! pattern are processed sequentially and written to the output directory.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use crate::config::parser::load_pipeline;
use crate::effects::Pipeline;
use crate::engine::export::{ExportFormat, export_image};

/// Arguments required for headless batch processing.
pub struct BatchArgs {
    /// Glob pattern used to discover input images (e.g. `images/*.png`).
    pub glob_pattern: String,
    /// Path to a JSON or YAML pipeline file.
    pub pipeline_path: PathBuf,
    /// Directory in which processed images are written.
    pub output_dir: PathBuf,
}

/// Run the batch processor.
///
/// 1. Loads the pipeline from `args.pipeline_path`.
/// 2. Expands `args.glob_pattern` to a list of input image paths.
/// 3. Creates `args.output_dir` if it does not exist.
/// 4. Processes every input image sequentially.
/// 5. Prints a summary and returns an error if any file failed.
pub fn run_batch(args: &BatchArgs) -> Result<()> {
    // Initialize WASM plugin registry so WASM effects work in batch mode.
    crate::effects::wasm::registry::init_registry();

    let pipeline = load_pipeline(&args.pipeline_path)
        .with_context(|| format!("loading pipeline from {}", args.pipeline_path.display()))?;

    let paths = collect_paths(&args.glob_pattern)?;

    if paths.is_empty() {
        eprintln!("No files matched the pattern: {}", args.glob_pattern);
        return Ok(());
    }

    std::fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("creating output directory {}", args.output_dir.display()))?;

    println!(
        "Spix batch mode: processing {} image(s) with pipeline \"{}\"",
        paths.len(),
        args.pipeline_path.display()
    );
    println!("Output directory: {}", args.output_dir.display());

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for src in &paths {
        match process_one(src, &pipeline, &args.output_dir) {
            Ok(dst) => {
                println!("  ✓  {}  →  {}", src.display(), dst.display());
                succeeded += 1;
            }
            Err(e) => {
                eprintln!("  ✗  {}  →  {e}", src.display());
                failed += 1;
            }
        }
    }

    println!("\nDone: {succeeded} succeeded, {failed} failed.");

    if failed > 0 {
        bail!("{failed} file(s) failed to process");
    }

    Ok(())
}

/// Expand a glob pattern into a sorted list of existing file paths.
fn collect_paths(pattern: &str) -> Result<Vec<PathBuf>> {
    let entries =
        glob::glob(pattern).with_context(|| format!("invalid glob pattern: {pattern}"))?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| match entry {
            Ok(p) if p.is_file() => Some(p),
            Ok(_) => None,
            Err(e) => {
                eprintln!("  Warning: skipping unreadable path — {e}");
                None
            }
        })
        .collect();

    paths.sort();
    Ok(paths)
}

/// Load, process, and export a single image.
fn process_one(src: &Path, pipeline: &Pipeline, output_dir: &Path) -> Result<PathBuf> {
    let img = image::open(src).with_context(|| format!("opening {}", src.display()))?;

    let processed = pipeline.apply_image(img);

    let filename = src
        .file_name()
        .with_context(|| format!("source path has no filename: {}", src.display()))?;

    let dst = output_dir.join(filename);
    let format = format_for_path(&dst);

    export_image(&processed, dst, &format).with_context(|| format!("exporting {}", src.display()))
}

/// Choose an [`ExportFormat`] based on the file extension of `path`.
/// Falls back to [`ExportFormat::Png`] for unrecognised extensions.
fn format_for_path(path: &Path) -> ExportFormat {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => ExportFormat::Jpeg { quality: 90 },
        Some("webp") => ExportFormat::WebP,
        Some("bmp") => ExportFormat::Bmp,
        _ => ExportFormat::Png,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{Effect, EnabledEffect, Pipeline, color::ColorEffect};
    use std::fs;
    use tempfile::TempDir;

    fn make_pipeline() -> Pipeline {
        Pipeline {
            effects: vec![EnabledEffect::new(Effect::Color(ColorEffect::Invert))],
        }
    }

    #[test]
    fn format_for_path_png() {
        assert!(matches!(
            format_for_path(Path::new("out.png")),
            ExportFormat::Png
        ));
    }

    #[test]
    fn format_for_path_jpeg() {
        assert!(matches!(
            format_for_path(Path::new("out.jpg")),
            ExportFormat::Jpeg { .. }
        ));
        assert!(matches!(
            format_for_path(Path::new("out.JPEG")),
            ExportFormat::Jpeg { .. }
        ));
    }

    #[test]
    fn format_for_path_unknown_falls_back_to_png() {
        assert!(matches!(
            format_for_path(Path::new("out.tiff")),
            ExportFormat::Png
        ));
    }

    #[test]
    fn collect_paths_invalid_pattern() {
        // Deliberately broken glob (unmatched `[`)
        let result = collect_paths("[invalid");
        assert!(result.is_err());
    }

    #[test]
    fn collect_paths_no_matches() {
        let result = collect_paths("/tmp/__no_files_here_spix_test_xyz/*.png").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn process_one_inverts_image() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("test.png");

        // Create a tiny black image (all zeros).
        let img = image::RgbImage::new(4, 4);
        let dyn_img = image::DynamicImage::ImageRgb8(img);
        dyn_img
            .save_with_format(&src, image::ImageFormat::Png)
            .unwrap();

        let pipeline = make_pipeline();
        let out_path = process_one(&src, &pipeline, tmp.path()).unwrap();

        assert!(out_path.exists());
        let out_img = image::open(&out_path).unwrap().into_rgb8();
        // Inverted black (0,0,0) → white (255,255,255)
        let px = out_img.get_pixel(0, 0);
        assert_eq!(px.0, [255, 255, 255]);
    }

    #[test]
    fn run_batch_processes_images() {
        let src_dir = TempDir::new().unwrap();
        let out_dir = TempDir::new().unwrap();

        // Create two tiny PNG files.
        for name in &["a.png", "b.png"] {
            let img = image::RgbImage::new(4, 4);
            let dyn_img = image::DynamicImage::ImageRgb8(img);
            dyn_img
                .save_with_format(src_dir.path().join(name), image::ImageFormat::Png)
                .unwrap();
        }

        // Save pipeline to a temp file.
        let pipeline = make_pipeline();
        let pipeline_path = src_dir.path().join("pipeline.json");
        let json = serde_json::to_string_pretty(&pipeline).unwrap();
        fs::write(&pipeline_path, json).unwrap();

        let pattern = format!("{}/*.png", src_dir.path().display());
        let args = BatchArgs {
            glob_pattern: pattern,
            pipeline_path,
            output_dir: out_dir.path().to_path_buf(),
        };

        run_batch(&args).unwrap();

        assert!(out_dir.path().join("a.png").exists());
        assert!(out_dir.path().join("b.png").exists());
    }
}
