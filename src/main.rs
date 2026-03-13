use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

mod app;
mod batch;
mod config;
mod debug;
mod effects;
mod engine;
mod ui;

fn main() -> Result<()> {
    // Parse CLI flags
    let args: Vec<String> = std::env::args().collect();
    let debug_mode = args.iter().any(|a| a == "--debug");

    if debug_mode {
        debug::init()?;
        log::info!("Spix starting (debug mode)");
    }

    // ── Batch mode ──────────────────────────────────────────────────────────
    // Activated when any of --batch / --pipeline / --outdir is present.
    // All three flags are required together.
    let batch_glob = flag_value(&args, "--batch");
    let pipeline_arg = flag_value(&args, "--pipeline");
    let outdir_arg = flag_value(&args, "--outdir");

    if batch_glob.is_some() || pipeline_arg.is_some() || outdir_arg.is_some() {
        let glob_pattern = batch_glob
            .context("--batch <glob> is required for batch mode")?
            .to_owned();
        let pipeline_path = pipeline_arg
            .context("--pipeline <file> is required for batch mode")?
            .into();
        let output_dir = outdir_arg
            .context("--outdir <dir> is required for batch mode")?
            .into();

        return batch::run_batch(&batch::BatchArgs {
            glob_pattern,
            pipeline_path,
            output_dir,
        });
    }

    // ── Interactive TUI mode ─────────────────────────────────────────────────

    // Install panic hook to restore terminal state before printing the trace
    std::panic::set_hook(Box::new(|info| {
        log::error!("PANIC: {info}");
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        eprintln!("Panic: {info}");
    }));

    // Set up terminal
    log::debug!("Setting up terminal (raw mode + alternate screen)");
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the application
    log::info!("Entering main loop");
    let result = app::run(&mut terminal);

    // Restore terminal
    log::debug!("Restoring terminal");
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    log::info!("Spix exiting");
    result
}

/// Return the value that follows `flag` in `args`, if present.
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].as_str())
}

#[cfg(test)]
mod snapshot_tests {
    use ratatui::{Terminal, backend::TestBackend};
    use ratatui_image::picker::Picker;
    use std::sync::mpsc;

    use crate::app::AppState;
    use crate::effects::{
        Effect, EnabledEffect, Pipeline, color::ColorEffect, glitch::GlitchEffect,
    };

    fn make_state() -> AppState {
        let (worker_tx, _worker_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let picker = Picker::halfblocks();
        AppState::new(worker_tx, resp_rx, resp_tx, picker)
    }

    #[test]
    fn render_initial_state_snapshot() {
        let mut state = make_state();
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut state))
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let snapshot: String = (0..30u16)
            .map(|y| {
                (0..100u16)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(snapshot.contains("Spix"), "Should show app name");
        assert!(snapshot.contains("Canvas"), "Should show Canvas panel");
        assert!(snapshot.contains("Effects"), "Should show Effects panel");
        assert!(snapshot.contains("Controls"), "Should show Controls panel");
    }

    #[test]
    fn render_with_effects_in_panel() {
        let mut state = make_state();
        state.pipeline = Pipeline {
            effects: vec![
                EnabledEffect::new(Effect::Color(ColorEffect::Invert)),
                EnabledEffect::new(Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })),
            ],
        };

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::ui::render(frame, &mut state))
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let snapshot: String = (0..30u16)
            .map(|y| {
                (0..100u16)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            snapshot.contains("Invert"),
            "Should show Invert in effects list"
        );
        assert!(
            snapshot.contains("Pixelate"),
            "Should show Pixelate in effects list"
        );
    }
}
