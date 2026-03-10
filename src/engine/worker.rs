use std::sync::mpsc::Receiver;

use crate::effects::Pipeline;

/// Commands sent from the UI thread to the worker thread.
pub enum WorkerCommand {
    /// Process the given image through the supplied pipeline.
    Process {
        image_path: std::path::PathBuf,
        pipeline: Pipeline,
    },
    /// Export the current processed image to a file.
    Export { output_path: std::path::PathBuf },
    /// Shut down the worker thread.
    Quit,
}

/// Worker thread entry point. Receives commands and dispatches work.
pub fn run(rx: Receiver<WorkerCommand>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::Process { image_path, pipeline } => {
                if let Err(e) = process_image(image_path, pipeline) {
                    eprintln!("Worker processing error: {e}");
                }
            }
            WorkerCommand::Export { output_path } => {
                if let Err(e) = crate::engine::export::export_image(output_path) {
                    eprintln!("Worker export error: {e}");
                }
            }
            WorkerCommand::Quit => break,
        }
    }
}

fn process_image(
    image_path: std::path::PathBuf,
    pipeline: Pipeline,
) -> anyhow::Result<()> {
    use rayon::prelude::*;

    let img = image::open(&image_path)?;
    let pixels: Vec<image::Rgba<u8>> = img
        .to_rgba8()
        .pixels()
        .copied()
        .collect();

    // Apply each effect in the pipeline in parallel, producing the processed frame.
    // In a full implementation this result would be sent back to the UI thread
    // (e.g. via a response channel) for rendering via ratatui-image.
    let _processed: Vec<image::Rgba<u8>> = pixels
        .par_iter()
        .map(|p| pipeline.apply_pixel(*p))
        .collect();

    Ok(())
}
