use std::sync::mpsc::{Receiver, Sender};

use crate::effects::Pipeline;

/// Commands sent from the UI thread to the worker thread.
pub enum WorkerCommand {
    /// Process the given image through the supplied pipeline.
    Process {
        image_path: std::path::PathBuf,
        pipeline: Pipeline,
        /// Channel on which to deliver the processed frame.
        response_tx: Sender<WorkerResponse>,
    },
    /// Export the given image to a file.
    Export {
        image: image::DynamicImage,
        output_path: std::path::PathBuf,
        response_tx: Sender<WorkerResponse>,
    },
    /// Shut down the worker thread.
    Quit,
}

/// Responses sent from the worker thread back to the UI thread.
pub enum WorkerResponse {
    /// A processed frame ready for display.
    ProcessedFrame(image::DynamicImage),
    /// An image was successfully exported to the given path.
    Exported(std::path::PathBuf),
    /// A human-readable error description.
    Error(String),
}

/// Worker thread entry point. Receives commands and dispatches work.
pub fn run(rx: Receiver<WorkerCommand>) {
    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::Process { image_path, pipeline, response_tx } => {
                if let Err(e) = process_image(image_path, pipeline, response_tx.clone()) {
                    let _ = response_tx.send(WorkerResponse::Error(e.to_string()));
                }
            }
            WorkerCommand::Export { image, output_path, response_tx } => {
                match crate::engine::export::export_image(&image, output_path.clone()) {
                    Ok(()) => {
                        let _ = response_tx.send(WorkerResponse::Exported(output_path));
                    }
                    Err(e) => {
                        let _ = response_tx.send(WorkerResponse::Error(e.to_string()));
                    }
                }
            }
            WorkerCommand::Quit => break,
        }
    }
}

fn process_image(
    image_path: std::path::PathBuf,
    pipeline: Pipeline,
    response_tx: Sender<WorkerResponse>,
) -> anyhow::Result<()> {
    let img = image::open(&image_path)?;

    // Apply the full pipeline (each effect may operate on the whole image).
    let result = pipeline.apply_image(img);

    let _ = response_tx.send(WorkerResponse::ProcessedFrame(result));
    Ok(())
}
