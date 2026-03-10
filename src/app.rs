use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{backend::Backend, Terminal};
use std::sync::mpsc;
use std::time::Duration;

use crate::effects::Pipeline;
use crate::engine::worker::WorkerCommand;

/// Central application state
pub struct AppState {
    /// Whether the application should quit
    pub should_quit: bool,
    /// The current effect pipeline
    pub pipeline: Pipeline,
    /// Path to the currently loaded image
    pub image_path: Option<std::path::PathBuf>,
    /// Sender channel to the worker thread
    pub worker_tx: mpsc::Sender<WorkerCommand>,
}

impl AppState {
    pub fn new(worker_tx: mpsc::Sender<WorkerCommand>) -> Self {
        Self {
            should_quit: false,
            pipeline: Pipeline::default(),
            image_path: None,
            worker_tx,
        }
    }
}

/// Entry point for the application event loop.
pub fn run<B: Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    // Set up the worker thread channel
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerCommand>();

    // Spawn the worker thread
    let worker_handle = std::thread::spawn(move || {
        crate::engine::worker::run(worker_rx);
    });

    let mut state = AppState::new(worker_tx);

    loop {
        // Render current state
        terminal.draw(|frame| {
            crate::ui::render(frame, &state);
        })?;

        // Handle input events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            state.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        if state.should_quit {
            // Signal the worker to stop
            let _ = state.worker_tx.send(WorkerCommand::Quit);
            break;
        }
    }

    worker_handle.join().ok();
    Ok(())
}
