use std::sync::mpsc::{Receiver, Sender};

use crate::effects::Pipeline;
use crate::engine::export::ExportFormat;

/// Commands sent from the UI thread to the worker thread.
pub enum WorkerCommand {
    /// Process the given image through the supplied pipeline.
    Process {
        /// Pre-decoded proxy image – the worker never touches disk during normal editing.
        image: image::DynamicImage,
        pipeline: Pipeline,
        /// Channel on which to deliver the processed frame.
        response_tx: Sender<WorkerResponse>,
    },
    /// Export the given image to a file.
    Export {
        image: image::DynamicImage,
        output_path: std::path::PathBuf,
        format: ExportFormat,
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
    log::info!("Worker thread started");
    // `pending` holds a non-Process command that was discovered while draining
    // stale Process jobs; it will be handled at the top of the next iteration.
    let mut pending: Option<WorkerCommand> = None;

    loop {
        let cmd = if let Some(p) = pending.take() {
            p
        } else {
            match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            }
        };

        match cmd {
            WorkerCommand::Process {
                image,
                pipeline,
                response_tx,
            } => {
                // Drain any additional Process commands queued since this one
                // arrived so that only the most-recent user intent is executed.
                let mut latest_image = image;
                let mut latest_pipeline = pipeline;
                let mut latest_resp_tx = response_tx;

                loop {
                    match rx.try_recv() {
                        Ok(WorkerCommand::Process {
                            image: img,
                            pipeline: pipe,
                            response_tx: tx,
                        }) => {
                            latest_image = img;
                            latest_pipeline = pipe;
                            latest_resp_tx = tx;
                        }
                        Ok(other) => {
                            // Non-Process command (Export, Quit) — defer it.
                            pending = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }

                // Apply the full pipeline (each effect may operate on the whole image).
                log::debug!(
                    "Worker: applying pipeline ({} effects) to {}x{} image",
                    latest_pipeline.effects.len(),
                    latest_image.width(),
                    latest_image.height()
                );
                let start = std::time::Instant::now();
                let result = latest_pipeline.apply_image(latest_image);
                log::debug!("Worker: pipeline applied in {:?}", start.elapsed());
                let _ = latest_resp_tx.send(WorkerResponse::ProcessedFrame(result));
            }
            WorkerCommand::Export {
                image,
                output_path,
                format,
                response_tx,
            } => {
                log::info!("Worker: exporting to {}", output_path.display());
                match crate::engine::export::export_image(&image, output_path, &format) {
                    Ok(saved_path) => {
                        log::info!("Worker: export succeeded → {}", saved_path.display());
                        let _ = response_tx.send(WorkerResponse::Exported(saved_path));
                    }
                    Err(e) => {
                        log::error!("Worker: export failed: {e}");
                        let _ = response_tx.send(WorkerResponse::Error(e.to_string()));
                    }
                }
            }
            WorkerCommand::Quit => {
                log::info!("Worker thread shutting down");
                break;
            }
        }
    }
}
