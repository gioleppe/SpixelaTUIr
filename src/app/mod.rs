//! Application module — state management, event loop, and keyboard handlers.
//!
//! This module was decomposed from a single 1,800-line `app.rs` into focused
//! sub-modules for maintainability:
//!
//! * [`state`] — `AppState` and its methods (image loading, undo, preview).
//! * [`handlers`] — Keyboard event handlers for every `InputMode`.
//! * [`dialogs`] — Dialog and modal state types (`ExportDialogState`, etc.).
//! * [`file_browser`] — `FileBrowserState` with filesystem navigation.
//! * [`pipeline_utils`] — Effect catalogue, randomisation, and formatting.
//! * [`animation`] — Animation data model (frames, timeline, playback).

pub mod animation;
pub mod dialogs;
pub mod file_browser;
pub mod handlers;
pub mod pipeline_utils;
pub mod state;

// ── Re-exports ──────────────────────────────────────────────────────────────
//
// All public items that the rest of the crate (`ui`, `main`, tests) referenced
// via `crate::app::Foo` are re-exported here so that no import paths need to
// change outside this module.

pub use dialogs::{FocusedPanel, InputMode};
pub use file_browser::{FileBrowserEntry, FileBrowserPurpose};
pub use pipeline_utils::AVAILABLE_EFFECTS;
pub use state::AppState;

// Re-exports used only by the test module (handler functions called directly).
#[cfg(test)]
pub use handlers::{handle_help_modal, handle_normal, handle_save_pipeline_dialog};

use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{Terminal, backend::Backend};
use ratatui_image::picker::Picker;

use crate::engine::worker::{WorkerCommand, WorkerResponse};

/// Keyboard hint shown in the controls bar and inside the file-browser footer.
pub const FILE_BROWSER_HINT: &str = "↑↓/jk: navigate  Enter: open  Backspace/-: up  Esc: cancel";

/// Keyboard hint shown when the file browser is open to load a pipeline.
pub const PIPELINE_BROWSER_HINT: &str =
    "↑↓/jk: navigate  Enter: load  Backspace/-: up  Esc: cancel";

/// Available proxy resolution tiers (max pixels on the long edge).
/// Index 1 (512 px) is the default — matches the previous hardcoded value.
pub const PROXY_RESOLUTIONS: &[u32] = &[256, 512, 768, 1024];

/// Entry point for the application event loop.
pub fn run<B: Backend>(terminal: &mut Terminal<B>) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let (worker_tx, worker_cmd_rx) = mpsc::channel::<WorkerCommand>();
    let (worker_resp_tx, worker_rx) = mpsc::channel::<WorkerResponse>();

    let worker_handle = std::thread::spawn(move || {
        crate::engine::worker::run(worker_cmd_rx);
    });

    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut state = AppState::new(worker_tx, worker_rx, worker_resp_tx, picker);

    // Always draw the very first frame so the UI is visible on startup.
    let mut ui_needs_redraw = true;

    loop {
        while let Ok(response) = state.worker_rx.try_recv() {
            match response {
                WorkerResponse::ProcessedFrame(img) => {
                    log::debug!("Received processed frame from worker");
                    let label = state
                        .image_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    state.set_preview(img);
                    state.status_message = format!("Ready | {label}");
                }
                WorkerResponse::AnimationFrameReady { frame_idx, image } => {
                    log::debug!("Received animation frame {frame_idx} from worker");
                    state.receive_animation_frame(frame_idx, image);
                    // If this is the currently selected frame, update the canvas.
                    if state.animation_panel_open
                        && frame_idx == state.animation.selected
                        && !state.animation_playback.is_playing()
                    {
                        if let Some(Some(img)) = state.animation_rendered_frames.get(frame_idx) {
                            let img = img.clone();
                            state.set_preview(img);
                        }
                    }
                    let pending = state.animation_pending_renders;
                    let total = state.animation.frames.len();
                    if pending == 0 {
                        state.status_message = format!(
                            "Animation: {total} frame{} ready.",
                            if total == 1 { "" } else { "s" }
                        );
                    } else {
                        state.status_message =
                            format!("Rendering animation frames… {}/{total}", total - pending);
                    }
                }
                WorkerResponse::SweepBatchReady { pipelines, images } => {
                    log::info!("Received sweep batch ({} frames)", images.len());
                    let count = images.len();
                    state.apply_sweep_results(pipelines, images);
                    state.status_message = format!(
                        "Sweep complete: {count} frame{} generated.",
                        if count == 1 { "" } else { "s" }
                    );
                }
                WorkerResponse::FileBrowserPreview(img) => {
                    log::debug!(
                        "Received file-browser preview ({}x{})",
                        img.width(),
                        img.height()
                    );
                    state.file_browser_preview = Some(state.picker.new_resize_protocol(img));
                }
                WorkerResponse::Exported(path) => {
                    log::info!("Export complete: {}", path.display());
                    state.status_message = format!("Exported → {}", path.display());
                }
                WorkerResponse::Error(e) => {
                    log::error!("Engine error: {e}");
                    state.status_message = format!("Engine error: {e}");
                }
            }
            // Any worker response means visible state changed.
            ui_needs_redraw = true;
        }

        // ── Animation playback tick ──────────────────────────────────────────
        if let animation::AnimationPlaybackState::Playing {
            current_frame,
            frame_started,
        } = &state.animation_playback
        {
            let current_frame = *current_frame;
            let frame_started = *frame_started;
            let elapsed_ms = frame_started.elapsed().as_millis() as u32;
            let duration_ms = state.animation.frame_duration_ms(current_frame);
            if elapsed_ms >= duration_ms {
                // Advance to the next frame.
                if let Some(next) = state.animation.next_frame(current_frame) {
                    state.animation_playback = animation::AnimationPlaybackState::Playing {
                        current_frame: next,
                        frame_started: std::time::Instant::now(),
                    };
                    // Show the next frame in the canvas.
                    if let Some(Some(img)) = state.animation_rendered_frames.get(next) {
                        let img = img.clone();
                        state.set_preview(img);
                    }
                    state.animation.selected = next;
                } else {
                    // End of animation, not looping.
                    state.animation_playback = animation::AnimationPlaybackState::Stopped;
                    state.status_message = "Animation playback finished.".to_string();
                }
                ui_needs_redraw = true;
            }
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // Track whether we're in a modal before the key is handled.
                    // When a modal closes, the Sixel image area will contain SKIP
                    // cells whose previous-buffer counterparts held modal text/borders.
                    // ratatui's diff skips SKIP cells in the new buffer, so those
                    // characters would never be cleared and the modal would ghost.
                    // Calling terminal.clear() resets ratatui's previous-buffer to
                    // all-default cells, forcing a full re-diff on the next draw.
                    let was_in_modal = state.input_mode.is_modal();
                    handlers::handle_key(&mut state, key.code, key.modifiers);
                    let is_in_modal = state.input_mode.is_modal();
                    
                    // If we were in a modal and either exited to a non-modal state
                    // OR transitioned to a different modal, we should clear the terminal
                    // to prevent artifacts from the previous modal.
                    if was_in_modal && !is_in_modal {
                        terminal.clear()?;
                    }
                    ui_needs_redraw = true;
                }
                // Terminal was resized — ratatui-image needs to re-encode at the
                // new dimensions, so force a full redraw.
                Event::Resize(_, _) => {
                    ui_needs_redraw = true;
                }
                _ => {}
            }
        }

        if ui_needs_redraw {
            terminal.draw(|frame| {
                crate::ui::render(frame, &mut state);
            })?;
            ui_needs_redraw = false;
        }

        if state.should_quit {
            let _ = state.worker_tx.send(WorkerCommand::Quit);
            break;
        }
    }

    worker_handle.join().ok();
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{
        Effect, EnabledEffect, Pipeline, color::ColorEffect, glitch::GlitchEffect,
    };
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn randomize_pipeline_populates_empty_pipeline() {
        let mut pipeline = Pipeline::default();
        assert!(pipeline.effects.is_empty(), "pipeline should start empty");

        pipeline_utils::randomize_pipeline(&mut pipeline);

        let len = pipeline.effects.len();
        assert!(
            (2..=5).contains(&len),
            "randomize should add 2–5 effects, got {len}"
        );
    }

    #[test]
    fn randomize_pipeline_replaces_existing_effects() {
        let mut pipeline = Pipeline::default();
        pipeline_utils::randomize_pipeline(&mut pipeline);
        let first_len = pipeline.effects.len();

        // A second call must also produce a valid count (pipeline is non-empty now).
        pipeline_utils::randomize_pipeline(&mut pipeline);
        let second_len = pipeline.effects.len();

        assert!(
            (2..=5).contains(&first_len),
            "first randomize should give 2–5 effects, got {first_len}"
        );
        assert!(
            (2..=5).contains(&second_len),
            "second randomize should give 2–5 effects, got {second_len}"
        );
    }

    fn make_state_with_effects() -> AppState {
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        let mut state = AppState::new(worker_tx, resp_rx, resp_tx, picker);
        state.pipeline = Pipeline {
            effects: vec![
                EnabledEffect::new(Effect::Color(ColorEffect::Invert)),
                EnabledEffect::new(Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })),
                EnabledEffect::new(Effect::Color(ColorEffect::HueShift { degrees: 30.0 })),
            ],
        };
        state.focused_panel = FocusedPanel::EffectsList;
        state.selected_effect = 1;
        state
    }

    #[test]
    fn move_effect_up_with_k() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('K'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        assert!(matches!(
            state.pipeline.effects[0].effect,
            Effect::Glitch(_)
        ));
        assert!(matches!(
            state.pipeline.effects[1].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_up_with_shift_up() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 0);
        assert!(matches!(
            state.pipeline.effects[0].effect,
            Effect::Glitch(_)
        ));
        assert!(matches!(
            state.pipeline.effects[1].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_down_with_j() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('J'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1].effect, Effect::Color(_)));
        assert!(matches!(
            state.pipeline.effects[2].effect,
            Effect::Glitch(_)
        ));
        assert!(state.status_message.contains("down"));
    }

    #[test]
    fn move_effect_down_with_shift_down() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Down, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1].effect, Effect::Color(_)));
        assert!(matches!(
            state.pipeline.effects[2].effect,
            Effect::Glitch(_)
        ));
        assert!(state.status_message.contains("down"));
    }

    #[test]
    fn move_effect_up_noop_at_first() {
        let mut state = make_state_with_effects();
        state.selected_effect = 0;
        let effects_before = state.pipeline.effects.clone();
        handle_normal(&mut state, KeyCode::Char('K'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        assert_eq!(state.pipeline.effects.len(), effects_before.len());
    }

    #[test]
    fn move_effect_down_noop_at_last() {
        let mut state = make_state_with_effects();
        state.selected_effect = 2;
        let effects_before = state.pipeline.effects.clone();
        handle_normal(&mut state, KeyCode::Char('J'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 2);
        assert_eq!(state.pipeline.effects.len(), effects_before.len());
    }

    #[test]
    fn plain_up_does_not_swap() {
        let mut state = make_state_with_effects();
        // Plain Up (no SHIFT) should only move cursor, not reorder effects.
        handle_normal(&mut state, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        // First effect must remain the original Invert, not the Pixelate.
        assert!(matches!(
            state.pipeline.effects[0].effect,
            Effect::Color(ColorEffect::Invert)
        ));
    }

    // ── Pipeline save / load ──────────────────────────────────────────────────

    fn make_state_empty() -> AppState {
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        AppState::new(worker_tx, resp_rx, resp_tx, picker)
    }

    #[test]
    fn ctrl_s_on_empty_pipeline_shows_error() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::Normal, "mode must stay Normal");
        assert!(
            state.status_message.contains("empty"),
            "status should mention empty pipeline: {}",
            state.status_message
        );
    }

    #[test]
    fn ctrl_s_with_effects_enters_save_dialog() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::SavePipelineDialog);
        // Dialog should be pre-populated with a non-empty default filename.
        assert!(
            !state.save_pipeline_dialog.filename.is_empty(),
            "default filename should be populated"
        );
    }

    #[test]
    fn save_pipeline_dialog_esc_cancels() {
        let mut state = make_state_with_effects();
        state.input_mode = InputMode::SavePipelineDialog;
        state.save_pipeline_dialog.filename = "something".to_string();
        handle_save_pipeline_dialog(&mut state, KeyCode::Esc);
        assert_eq!(state.input_mode, InputMode::Normal);
        assert!(
            state.status_message.contains("cancel") || state.status_message.contains("Cancel"),
            "status should mention cancellation: {}",
            state.status_message
        );
    }

    #[test]
    fn save_pipeline_dialog_enter_returns_to_normal() {
        let mut state = make_state_with_effects();
        state.input_mode = InputMode::SavePipelineDialog;
        // Point the dialog at the temp dir so we don't pollute the project root.
        state.save_pipeline_dialog.directory = std::env::temp_dir().to_string_lossy().into_owned();
        // Leave filename empty — effective_filename() will fall back to "pipeline".
        handle_save_pipeline_dialog(&mut state, KeyCode::Enter);
        assert_eq!(state.input_mode, InputMode::Normal);
        // Clean up any file that was created.
        let _ = std::fs::remove_file(std::env::temp_dir().join("pipeline.json"));
    }

    #[test]
    fn save_pipeline_dialog_enforces_json_extension() {
        let tmp_dir = std::env::temp_dir();
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        let mut state = AppState::new(worker_tx, resp_rx, resp_tx, picker);
        state.pipeline = Pipeline {
            effects: vec![EnabledEffect::new(Effect::Color(ColorEffect::Invert))],
        };
        state.input_mode = InputMode::SavePipelineDialog;
        state.save_pipeline_dialog.directory = tmp_dir.to_string_lossy().into_owned();
        state.save_pipeline_dialog.filename = "test_enforce_ext".to_string();
        handle_save_pipeline_dialog(&mut state, KeyCode::Enter);
        // Resulting path must carry the .json extension.
        let expected = tmp_dir.join("test_enforce_ext.json");
        assert!(
            expected.exists(),
            "saved file should have .json extension at {}",
            expected.display()
        );
        let _ = std::fs::remove_file(&expected);
    }

    #[test]
    fn help_modal_opens_and_closes() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(
            state.input_mode,
            InputMode::HelpModal,
            "h should open HelpModal"
        );
        handle_help_modal(&mut state, KeyCode::Esc);
        assert_eq!(
            state.input_mode,
            InputMode::Normal,
            "Esc should close HelpModal"
        );
    }

    #[test]
    fn help_modal_closed_by_h() {
        let mut state = make_state_empty();
        state.input_mode = InputMode::HelpModal;
        handle_help_modal(&mut state, KeyCode::Char('h'));
        assert_eq!(state.input_mode, InputMode::Normal);
    }

    #[test]
    fn save_pipeline_roundtrip() {
        let tmp = std::env::temp_dir().join("spixelatuir_test_pipeline.json");
        let pipeline = Pipeline {
            effects: vec![
                EnabledEffect::new(Effect::Color(ColorEffect::Invert)),
                EnabledEffect::new(Effect::Glitch(GlitchEffect::Pixelate { block_size: 4 })),
            ],
        };

        crate::config::parser::save_pipeline(&pipeline, &tmp).expect("save should succeed");

        let loaded = crate::config::parser::load_pipeline(&tmp).expect("load should succeed");

        assert_eq!(loaded.effects.len(), 2);
        assert!(matches!(
            loaded.effects[0].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(matches!(
            loaded.effects[1].effect,
            Effect::Glitch(GlitchEffect::Pixelate { block_size: 4 })
        ));
        assert!(
            loaded.effects[0].enabled,
            "loaded effects should be enabled by default"
        );
        assert!(
            loaded.effects[1].enabled,
            "all loaded effects should be enabled by default"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn ctrl_l_enters_file_browser_with_pipeline_purpose() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::FileBrowser);
        assert!(
            state
                .file_browser
                .as_ref()
                .map(|fb| fb.purpose == FileBrowserPurpose::LoadPipeline)
                .unwrap_or(false),
            "file browser should have LoadPipeline purpose"
        );
    }

    #[test]
    fn ctrl_o_enters_file_browser_with_open_image_purpose() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(state.input_mode, InputMode::FileBrowser);
        assert!(
            state
                .file_browser
                .as_ref()
                .map(|fb| fb.purpose == FileBrowserPurpose::OpenImage)
                .unwrap_or(false),
            "file browser should have OpenImage purpose"
        );
    }
}
