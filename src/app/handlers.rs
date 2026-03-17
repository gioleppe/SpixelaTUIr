use crossterm::event::{KeyCode, KeyModifiers};

use crate::effects::{Effect, EnabledEffect, color, color::ColorEffect};
use crate::engine::export::EXPORT_FORMATS;

use super::PROXY_RESOLUTIONS;
use super::animation::{
    ANIM_EXPORT_FORMATS, AnimationPlaybackState, SWEEP_EASINGS, SweepDialogState, apply_easing,
};
use super::dialogs::{FocusedPanel, InputMode};
use super::file_browser::{FileBrowserEntry, FileBrowserPurpose, FileBrowserState};
use super::pipeline_utils::{AVAILABLE_EFFECTS, format_param_value, randomize_pipeline};
use super::state::AppState;

/// Top-level key dispatch — routes to the appropriate mode-specific handler.
pub fn handle_key(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    log::debug!(
        "Key press: {code:?} (modifiers: {modifiers:?}) in mode {:?}",
        state.input_mode
    );
    match state.input_mode {
        InputMode::Normal => handle_normal(state, code, modifiers),
        InputMode::PathInput => handle_path_input(state, code),
        InputMode::AddEffect => handle_add_effect(state, code),
        InputMode::FileBrowser => handle_file_browser(state, code),
        InputMode::EditEffect { .. } => handle_edit_effect(state, code),
        InputMode::ExportDialog => handle_export_dialog(state, code),
        InputMode::SavePipelineDialog => handle_save_pipeline_dialog(state, code),
        InputMode::HelpModal => handle_help_modal(state, code),
        InputMode::ConfirmClearPipeline => handle_confirm_clear_pipeline(state, code),
        InputMode::ConfirmQuit => handle_confirm_quit(state, code),
        InputMode::AnimationPanel => handle_animation_panel(state, code, modifiers),
        InputMode::AnimationSweepDialog => handle_animation_sweep_dialog(state, code),
        InputMode::AnimationExportDialog => handle_animation_export_dialog(state, code),
        InputMode::AnimationFrameDurationInput => {
            handle_animation_frame_duration_input(state, code)
        }
    }
}

// ── Normal mode ──────────────────────────────────────────────────────────────

pub fn handle_normal(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    // Any keypress ends the drag highlight by default; move_effect_* will re-enable it.
    state.dragging_effect = false;
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if state.pipeline_dirty {
                state.input_mode = InputMode::ConfirmQuit;
            } else {
                state.should_quit = true;
            }
        }
        KeyCode::Char('o') => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            state.file_browser = Some(FileBrowserState::new(cwd, FileBrowserPurpose::OpenImage));
            state.input_mode = InputMode::FileBrowser;
            dispatch_file_browser_preview_for_cursor(state);
        }
        KeyCode::Tab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Canvas => FocusedPanel::EffectsList,
                FocusedPanel::EffectsList => {
                    if state.animation_panel_open {
                        // Enter animation panel InputMode when Tab cycles to it.
                        state.input_mode = InputMode::AnimationPanel;
                        FocusedPanel::AnimationPanel
                    } else {
                        FocusedPanel::Canvas
                    }
                }
                FocusedPanel::AnimationPanel => {
                    state.input_mode = InputMode::Normal;
                    FocusedPanel::Canvas
                }
            };
        }
        // Move selected effect one position up (Shift+Up or K).
        KeyCode::Up
            if state.focused_panel == FocusedPanel::EffectsList
                && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            move_effect_up(state);
        }
        KeyCode::Char('K') if state.focused_panel == FocusedPanel::EffectsList => {
            move_effect_up(state);
        }
        // Move selected effect one position down (Shift+Down or J).
        KeyCode::Down
            if state.focused_panel == FocusedPanel::EffectsList
                && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            move_effect_down(state);
        }
        KeyCode::Char('J') if state.focused_panel == FocusedPanel::EffectsList => {
            move_effect_down(state);
        }
        // Effects-list navigation (when effects panel is focused) — circular wrapping.
        KeyCode::Up | KeyCode::Char('k') if state.focused_panel == FocusedPanel::EffectsList => {
            let len = state.pipeline.effects.len();
            if len > 0 {
                state.selected_effect = if state.selected_effect == 0 {
                    len - 1
                } else {
                    state.selected_effect - 1
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') if state.focused_panel == FocusedPanel::EffectsList => {
            let len = state.pipeline.effects.len();
            if len > 0 {
                state.selected_effect = if state.selected_effect >= len - 1 {
                    0
                } else {
                    state.selected_effect + 1
                };
            }
        }
        // Add effect.
        KeyCode::Char('a') if state.focused_panel == FocusedPanel::EffectsList => {
            state.input_mode = InputMode::AddEffect;
            state.add_effect_cursor = 0;
        }
        // Toggle the selected effect on/off without removing it (Space).
        KeyCode::Char(' ') if state.focused_panel == FocusedPanel::EffectsList => {
            if !state.pipeline.effects.is_empty() {
                let ee = &mut state.pipeline.effects[state.selected_effect];
                ee.enabled = !ee.enabled;
                let enabled = ee.enabled;
                state.pipeline_dirty = true;
                state.status_message = if enabled {
                    "Effect enabled. Re-processing…".to_string()
                } else {
                    "Effect disabled. Re-processing…".to_string()
                };
                state.image_protocol = None;
                state.dispatch_process();
            }
        }
        // Delete selected effect.
        KeyCode::Delete | KeyCode::Char('d')
            if state.focused_panel == FocusedPanel::EffectsList =>
        {
            if !state.pipeline.effects.is_empty() {
                state.push_undo();
                state.pipeline.effects.remove(state.selected_effect);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.image_protocol = None;
                state.dispatch_process();
                state.status_message = "Effect removed. Re-processing…".to_string();
            }
        }
        // Randomize effect parameters.
        KeyCode::Char('r') => {
            state.mutate_pipeline(randomize_pipeline);
            state.status_message = "Randomised pipeline. Re-processing…".to_string();
        }
        // Open the edit-parameters modal (effects panel focused, effect selected).
        KeyCode::Enter
            if state.focused_panel == FocusedPanel::EffectsList
                && !state.pipeline.effects.is_empty() =>
        {
            let descriptors = state.pipeline.effects[state.selected_effect]
                .effect
                .param_descriptors();
            if descriptors.is_empty() {
                state.status_message = "This effect has no editable parameters.".to_string();
            } else {
                state.edit_params = descriptors
                    .iter()
                    .map(|d| format_param_value(d.value))
                    .collect();
                state.input_mode = InputMode::EditEffect { field_idx: 0 };
                state.status_message =
                    "Edit parameters (↑↓: field, Enter: apply, Esc: cancel)".to_string();
            }
        }
        // Open export dialog.
        KeyCode::Char('e') => {
            if state.preview_buffer.is_some() {
                // Populate dialog defaults from the current image path.
                let default_filename = state
                    .image_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| format!("{}_out", s.to_string_lossy()))
                    .unwrap_or_else(|| "output".to_string());
                state.export_dialog.filename = default_filename;
                state.export_dialog.format_index = 0;
                state.export_dialog.focused_field = 1;
                state.input_mode = InputMode::ExportDialog;
            } else {
                state.status_message = "No processed image to export.".to_string();
            }
        }
        // Decrease preview resolution.
        KeyCode::Char('[') => {
            if state.source_asset.is_some() && state.proxy_resolution_index > 0 {
                state.proxy_resolution_index -= 1;
                state.reload_proxy();
            }
        }
        // Increase preview resolution.
        KeyCode::Char(']') => {
            if state.source_asset.is_some()
                && state.proxy_resolution_index < PROXY_RESOLUTIONS.len() - 1
            {
                state.proxy_resolution_index += 1;
                state.reload_proxy();
            }
        }
        // Save the current pipeline via the save-pipeline dialog (Ctrl+S).
        KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
            if state.pipeline.effects.is_empty() {
                state.status_message = "Pipeline is empty – nothing to save.".to_string();
            } else {
                // Pre-populate with a sensible default filename.
                let default_filename = state
                    .image_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "pipeline".to_string());
                state.save_pipeline_dialog.filename = default_filename;
                state.save_pipeline_dialog.focused_field = 1;
                state.input_mode = InputMode::SavePipelineDialog;
            }
        }
        // Undo last pipeline edit (Ctrl+Z).
        KeyCode::Char('z') if modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(previous) = state.undo_stack.pop_front() {
                let current = std::mem::replace(&mut state.pipeline, previous);
                state.redo_stack.push_front(current);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.image_protocol = None;
                state.dispatch_process();
                state.status_message = format!(
                    "Undo – {} effect{} in pipeline.",
                    state.pipeline.effects.len(),
                    if state.pipeline.effects.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            } else {
                state.status_message = "Nothing to undo.".to_string();
            }
        }
        // Redo last undone pipeline edit (Ctrl+Y).
        KeyCode::Char('y') if modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(next) = state.redo_stack.pop_front() {
                let current = std::mem::replace(&mut state.pipeline, next);
                state.undo_stack.push_front(current);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.image_protocol = None;
                state.dispatch_process();
                state.status_message = format!(
                    "Redo – {} effect{} in pipeline.",
                    state.pipeline.effects.len(),
                    if state.pipeline.effects.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            } else {
                state.status_message = "Nothing to redo.".to_string();
            }
        }
        // Load a pipeline from a JSON or YAML file via the file browser (Ctrl+L).
        KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            state.file_browser = Some(FileBrowserState::new(cwd, FileBrowserPurpose::LoadPipeline));
            state.input_mode = InputMode::FileBrowser;
            dispatch_file_browser_preview_for_cursor(state);
        }
        // Clear all effects with a confirmation prompt (Ctrl+D).
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !state.pipeline.effects.is_empty() {
                state.input_mode = InputMode::ConfirmClearPipeline;
                state.status_message =
                    "Clear all effects? Press Enter to confirm or Esc to cancel.".to_string();
            } else {
                state.status_message = "Pipeline is already empty.".to_string();
            }
        }
        // Open the full keyboard-shortcut help overlay.
        KeyCode::Char('h') => {
            state.input_mode = InputMode::HelpModal;
        }
        // Toggle live histogram overlay.
        KeyCode::Char('H') => {
            state.show_histogram = !state.show_histogram;
            state.status_message = if state.show_histogram {
                "Histogram overlay enabled.".to_string()
            } else {
                "Histogram overlay disabled.".to_string()
            };
        }
        // Toggle side-by-side before/after split view.
        KeyCode::Char('v') => {
            state.split_view = !state.split_view;
            state.status_message = if state.split_view {
                "Split view enabled – left: original, right: processed.".to_string()
            } else {
                "Split view disabled.".to_string()
            };
        }
        // Toggle animation panel (Ctrl+N).
        KeyCode::Char('n') if modifiers.contains(KeyModifiers::CONTROL) => {
            state.animation_panel_open = !state.animation_panel_open;
            if state.animation_panel_open {
                state.status_message = "Animation panel opened. Tab to focus it.".to_string();
            } else {
                // Make sure we leave animation-panel focus when closing.
                if matches!(state.focused_panel, FocusedPanel::AnimationPanel) {
                    state.focused_panel = FocusedPanel::Canvas;
                }
                // Stop any active playback.
                state.animation_playback = AnimationPlaybackState::Stopped;
                state.status_message = "Animation panel closed.".to_string();
            }
        }
        _ => {}
    }
}

/// Move the selected effect one position up in the pipeline.
fn move_effect_up(state: &mut AppState) {
    let idx = state.selected_effect;
    if idx > 0 {
        state.push_undo();
        state.pipeline.effects.swap(idx, idx - 1);
        state.selected_effect -= 1;
        state.dragging_effect = true;
        state.pipeline_dirty = true;
        state.status_message = "Moved effect up. Re-processing…".to_string();
        state.image_protocol = None;
        state.dispatch_process();
    }
}

/// Move the selected effect one position down in the pipeline.
fn move_effect_down(state: &mut AppState) {
    let idx = state.selected_effect;
    let last = state.pipeline.effects.len().saturating_sub(1);
    if idx < last {
        state.push_undo();
        state.pipeline.effects.swap(idx, idx + 1);
        state.selected_effect += 1;
        state.dragging_effect = true;
        state.pipeline_dirty = true;
        state.status_message = "Moved effect down. Re-processing…".to_string();
        state.image_protocol = None;
        state.dispatch_process();
    }
}

// ── Path input ───────────────────────────────────────────────────────────────

fn handle_path_input(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.path_input.clear();
            state.status_message = "Cancelled.".to_string();
        }
        KeyCode::Enter => {
            let path = std::path::PathBuf::from(state.path_input.trim());
            state.input_mode = InputMode::Normal;
            state.path_input.clear();
            state.load_image(path);
        }
        KeyCode::Backspace => {
            state.path_input.pop();
        }
        KeyCode::Char(c) => {
            state.path_input.push(c);
        }
        _ => {}
    }
}

// ── Add effect menu ──────────────────────────────────────────────────────────

fn handle_add_effect(state: &mut AppState, code: KeyCode) {
    let n = AVAILABLE_EFFECTS.len();
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.add_effect_cursor = if state.add_effect_cursor == 0 {
                n - 1
            } else {
                state.add_effect_cursor - 1
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.add_effect_cursor = if state.add_effect_cursor >= n - 1 {
                0
            } else {
                state.add_effect_cursor + 1
            };
        }
        KeyCode::Enter => {
            let effect = AVAILABLE_EFFECTS[state.add_effect_cursor].2();
            state.push_undo();
            state.pipeline.effects.push(EnabledEffect::new(effect));

            let last_idx = state.pipeline.effects.len() - 1;
            state.selected_effect = last_idx;
            state.pipeline_dirty = true;

            let descriptors = state.pipeline.effects[last_idx].effect.param_descriptors();
            if !descriptors.is_empty() {
                // If the effect has parameters (like GradientMap), open the edit modal immediately.
                state.edit_params = descriptors
                    .iter()
                    .map(|d| format_param_value(d.value))
                    .collect();
                state.input_mode = InputMode::EditEffect { field_idx: 0 };
                state.status_message = format!(
                    "Added '{}' – edit parameters (Enter: apply, Esc: cancel)",
                    AVAILABLE_EFFECTS[state.add_effect_cursor].0
                );
            } else {
                state.input_mode = InputMode::Normal;
                state.status_message = format!(
                    "Added '{}'. Re-processing…",
                    AVAILABLE_EFFECTS[state.add_effect_cursor].0
                );
            }

            state.image_protocol = None;
            state.dispatch_process();
        }
        _ => {}
    }
}

// ── File browser ─────────────────────────────────────────────────────────────

fn handle_file_browser(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.file_browser = None;
            state.file_browser_preview = None;
            state.status_message = "Cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_up();
            }
            dispatch_file_browser_preview_for_cursor(state);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_down();
            }
            dispatch_file_browser_preview_for_cursor(state);
        }
        KeyCode::Backspace | KeyCode::Char('-') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.go_up();
            }
            // After navigating up, reset preview for new cursor position.
            dispatch_file_browser_preview_for_cursor(state);
        }
        KeyCode::Enter => {
            // Clone what we need first to avoid the borrow-checker conflict.
            let action = state
                .file_browser
                .as_ref()
                .and_then(|fb| fb.entries.get(fb.cursor).cloned());
            let purpose = state.file_browser.as_ref().map(|fb| fb.purpose.clone());
            match action {
                Some(FileBrowserEntry::Directory(_)) => {
                    if let Some(ref mut fb) = state.file_browser {
                        fb.enter_dir();
                    }
                    // After descending into a directory, update the preview.
                    dispatch_file_browser_preview_for_cursor(state);
                }
                Some(FileBrowserEntry::ImageFile(path, _)) => {
                    state.input_mode = InputMode::Normal;
                    state.file_browser = None;
                    state.file_browser_preview = None;
                    match purpose {
                        Some(FileBrowserPurpose::LoadPipeline) => {
                            match crate::config::parser::load_pipeline(&path) {
                                Ok(pipeline) => {
                                    let effect_count = pipeline.effects.len();
                                    let filename = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| path.display().to_string());
                                    state.pipeline = pipeline;
                                    state.clamp_selection();
                                    state.pipeline_dirty = false;
                                    state.undo_stack.clear();
                                    state.redo_stack.clear();
                                    state.image_protocol = None;
                                    state.dispatch_process();
                                    state.status_message = format!(
                                        "Loaded {effect_count} effect{} from {filename}",
                                        if effect_count == 1 { "" } else { "s" }
                                    );
                                }
                                Err(e) => {
                                    state.status_message = format!("Error loading pipeline: {e}");
                                }
                            }
                        }
                        _ => {
                            state.load_image(path);
                        }
                    }
                }
                None => {}
            }
        }
        _ => {}
    }
}

/// If the file browser cursor currently points at an image file **and the
/// browser was opened for image selection**, dispatch a thumbnail-load request
/// to the worker thread.  Otherwise clear any stale preview so the panel shows
/// the "no preview" placeholder instead.
fn dispatch_file_browser_preview_for_cursor(state: &mut AppState) {
    // Only load thumbnails when the browser is in OpenImage mode; the pipeline
    // browser does not show a preview pane so there is no need to dispatch.
    let is_open_image = state
        .file_browser
        .as_ref()
        .map(|fb| fb.purpose == FileBrowserPurpose::OpenImage)
        .unwrap_or(false);

    if !is_open_image {
        return;
    }

    let entry = state
        .file_browser
        .as_ref()
        .and_then(|fb| fb.entries.get(fb.cursor).cloned());
    match entry {
        Some(FileBrowserEntry::ImageFile(path, _)) => {
            state.dispatch_file_browser_preview(path);
        }
        _ => {
            state.file_browser_preview = None;
        }
    }
}

// ── Edit effect parameters ───────────────────────────────────────────────────

fn handle_edit_effect(state: &mut AppState, code: KeyCode) {
    let field_idx = match state.input_mode {
        InputMode::EditEffect { field_idx } => field_idx,
        _ => return,
    };

    let num_fields = if state.pipeline.effects.is_empty() {
        0
    } else {
        state.pipeline.effects[state.selected_effect]
            .effect
            .param_descriptors()
            .len()
    };

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.edit_params.clear();
            state.status_message = "Edit cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if num_fields > 0 {
                let new_idx = if field_idx == 0 {
                    num_fields - 1
                } else {
                    field_idx - 1
                };
                state.input_mode = InputMode::EditEffect { field_idx: new_idx };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if num_fields > 0 {
                let new_idx = if field_idx + 1 >= num_fields {
                    0
                } else {
                    field_idx + 1
                };
                state.input_mode = InputMode::EditEffect { field_idx: new_idx };
            }
        }
        KeyCode::Backspace => {
            if let Some(buf) = state.edit_params.get_mut(field_idx) {
                buf.pop();
            }
        }
        KeyCode::Left | KeyCode::Right => {
            if !state.pipeline.effects.is_empty() {
                let descriptors = state.pipeline.effects[state.selected_effect]
                    .effect
                    .param_descriptors();
                if let Some(d) = descriptors.get(field_idx) {
                    // For "preset" fields, we allow arrow-key cycling.
                    if d.name == "preset" {
                        let current = state.edit_params[field_idx]
                            .parse::<f32>()
                            .unwrap_or(d.value);
                        let n = color::GRADIENT_PRESETS.len() as f32;
                        let next = if matches!(code, KeyCode::Left) {
                            (current + n - 1.0) % n
                        } else {
                            (current + 1.0) % n
                        };
                        state.edit_params[field_idx] = format_param_value(next);

                        // Auto-apply preset changes immediately so the user sees the colors change.
                        let values: Vec<f32> = state
                            .edit_params
                            .iter()
                            .zip(descriptors.iter())
                            .map(|(s, d)| s.parse::<f32>().unwrap_or(d.value).clamp(d.min, d.max))
                            .collect();
                        let updated = state.pipeline.effects[state.selected_effect]
                            .effect
                            .apply_params(&values);

                        state.pipeline.effects[state.selected_effect].effect = updated;
                        state.image_protocol = None;
                        state.dispatch_process();

                        // Refresh edit_params in case new fields (RGB) were revealed.
                        let new_descriptors = state.pipeline.effects[state.selected_effect]
                            .effect
                            .param_descriptors();
                        state.edit_params = new_descriptors
                            .iter()
                            .map(|d| format_param_value(d.value))
                            .collect();
                    }
                }
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
            if let Some(buf) = state.edit_params.get_mut(field_idx) {
                buf.push(c);
            }
        }
        KeyCode::Enter => {
            if !state.pipeline.effects.is_empty() {
                let descriptors = state.pipeline.effects[state.selected_effect]
                    .effect
                    .param_descriptors();
                let values: Vec<f32> = state
                    .edit_params
                    .iter()
                    .zip(descriptors.iter())
                    .map(|(s, d)| s.parse::<f32>().unwrap_or(d.value).clamp(d.min, d.max))
                    .collect();

                let old_effect = &state.pipeline.effects[state.selected_effect].effect;
                let updated = old_effect.apply_params(&values);

                // If it's a GradientMap and the preset just changed to "Custom" (last one),
                // we keep the modal open so the user can edit the newly revealed RGB fields.
                let mut keep_open = false;
                if let (
                    Effect::Color(ColorEffect::GradientMap {
                        preset_idx: old_p, ..
                    }),
                    Effect::Color(ColorEffect::GradientMap {
                        preset_idx: new_p, ..
                    }),
                ) = (old_effect, &updated)
                    && *old_p != *new_p
                    && *new_p == color::GRADIENT_PRESETS.len() - 1
                {
                    keep_open = true;
                }

                state.push_undo();
                state.pipeline.effects[state.selected_effect].effect = updated;
                state.pipeline_dirty = true;
                state.status_message = "Effect updated. Re-processing…".to_string();
                state.image_protocol = None;
                state.dispatch_process();

                if keep_open {
                    // Refresh edit_params so the new RGB fields are visible.
                    let new_descriptors = state.pipeline.effects[state.selected_effect]
                        .effect
                        .param_descriptors();
                    state.edit_params = new_descriptors
                        .iter()
                        .map(|d| format_param_value(d.value))
                        .collect();
                    // Stay in EditEffect mode.
                    return;
                }
            }
            state.input_mode = InputMode::Normal;
            state.edit_params.clear();
        }
        _ => {}
    }
}

// ── Export dialog ─────────────────────────────────────────────────────────────

fn handle_export_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_DIRECTORY: usize = 0;
    const FIELD_FILENAME: usize = 1;
    const FIELD_FORMAT: usize = 2;
    const FIELD_COUNT: usize = 3;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Export cancelled.".to_string();
        }
        KeyCode::Enter => {
            // Build the output path from dialog state.
            let format = EXPORT_FORMATS[state.export_dialog.format_index].clone();
            let ext = format.extension();
            let dir = std::path::PathBuf::from(&state.export_dialog.directory);
            let filename = state.export_dialog.effective_filename().to_string();
            let output_path = dir.join(format!("{filename}.{ext}"));
            state.dispatch_export(output_path, format);
            state.input_mode = InputMode::Normal;
            state.status_message = "Exporting…".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.export_dialog.focused_field > 0 {
                state.export_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.export_dialog.focused_field < FIELD_COUNT - 1 {
                state.export_dialog.focused_field += 1;
            }
        }
        // Format cycling.
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if state.export_dialog.focused_field == FIELD_FORMAT =>
        {
            let n = EXPORT_FORMATS.len();
            state.export_dialog.format_index = match code {
                KeyCode::Left => {
                    if state.export_dialog.format_index == 0 {
                        n - 1
                    } else {
                        state.export_dialog.format_index - 1
                    }
                }
                _ => (state.export_dialog.format_index + 1) % n,
            };
        }
        // Text editing for Directory and Filename fields.
        KeyCode::Backspace => match state.export_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.export_dialog.directory.pop();
            }
            FIELD_FILENAME => {
                state.export_dialog.filename.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.export_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.export_dialog.directory.push(c);
            }
            FIELD_FILENAME => {
                state.export_dialog.filename.push(c);
            }
            _ => {}
        },
        _ => {}
    }
}

// ── Save pipeline dialog ─────────────────────────────────────────────────────

pub fn handle_save_pipeline_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_DIRECTORY: usize = 0;
    const FIELD_FILENAME: usize = 1;
    const FIELD_COUNT: usize = 2;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Save cancelled.".to_string();
        }
        KeyCode::Enter => {
            let dir = std::path::PathBuf::from(&state.save_pipeline_dialog.directory);
            let filename = state.save_pipeline_dialog.effective_filename().to_string();
            // Enforce the .json extension regardless of what the user typed.
            let output_path = dir.join(format!("{filename}.json"));
            match crate::config::parser::save_pipeline(&state.pipeline, &output_path) {
                Ok(()) => {
                    state.pipeline_dirty = false;
                    state.status_message = format!("Pipeline saved → {}", output_path.display());
                }
                Err(e) => {
                    state.status_message = format!("Error saving pipeline: {e}");
                }
            }
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.save_pipeline_dialog.focused_field > 0 {
                state.save_pipeline_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.save_pipeline_dialog.focused_field < FIELD_COUNT - 1 {
                state.save_pipeline_dialog.focused_field += 1;
            }
        }
        KeyCode::Backspace => match state.save_pipeline_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.save_pipeline_dialog.directory.pop();
            }
            FIELD_FILENAME => {
                state.save_pipeline_dialog.filename.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.save_pipeline_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.save_pipeline_dialog.directory.push(c);
            }
            FIELD_FILENAME => {
                state.save_pipeline_dialog.filename.push(c);
            }
            _ => {}
        },
        _ => {}
    }
}

// ── Help modal ───────────────────────────────────────────────────────────────

pub fn handle_help_modal(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
            state.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

// ── Confirm clear pipeline ───────────────────────────────────────────────────

fn handle_confirm_clear_pipeline(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            state.mutate_pipeline(|p| p.effects.clear());
            state.selected_effect = 0;
            state.input_mode = InputMode::Normal;
            state.status_message = "Pipeline cleared.".to_string();
        }
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Clear cancelled.".to_string();
        }
        _ => {}
    }
}

// ── Confirm quit ─────────────────────────────────────────────────────────────

fn handle_confirm_quit(state: &mut AppState, code: KeyCode) {
    match code {
        // 'y' or Enter: discard changes and quit.
        KeyCode::Char('y') | KeyCode::Enter => {
            state.should_quit = true;
        }
        // 'n' or Esc: go back to normal mode.
        KeyCode::Char('n') | KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Quit cancelled.".to_string();
        }
        // 's': save pipeline and quit.
        KeyCode::Char('s') => {
            state.input_mode = InputMode::SavePipelineDialog;
        }
        _ => {}
    }
}

// ── Animation panel ──────────────────────────────────────────────────────────

/// Handler for `InputMode::AnimationPanel` — the animation panel has focus.
fn handle_animation_panel(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        // ── Frame navigation ─────────────────────────────────────────────────
        KeyCode::Left | KeyCode::Char('h') => {
            if state.animation.selected > 0 {
                state.animation.selected -= 1;
                show_animation_frame(state, state.animation.selected);
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            let max = state.animation.frames.len().saturating_sub(1);
            if state.animation.selected < max {
                state.animation.selected += 1;
                let sel = state.animation.selected;
                show_animation_frame(state, sel);
            }
        }

        // ── Capture current pipeline as a new frame ──────────────────────────
        KeyCode::Char('c') => {
            if state.source_asset.is_none() {
                state.status_message = "No image loaded — open an image first.".to_string();
                return;
            }
            let idx = state.capture_animation_frame();
            let total = state.animation.frames.len();
            state.status_message = format!("Captured frame {}/{total}. Worker rendering…", idx + 1);
        }

        // ── Delete selected frame ─────────────────────────────────────────────
        KeyCode::Delete | KeyCode::Char('d') => {
            if !state.animation.frames.is_empty() {
                let idx = state.animation.selected;
                state.animation.frames.remove(idx);
                if idx < state.animation_rendered_frames.len() {
                    state.animation_rendered_frames.remove(idx);
                }
                state.clamp_animation_selection();
                let sel = state.animation.selected;
                show_animation_frame(state, sel);
                let total = state.animation.frames.len();
                state.status_message = format!(
                    "Frame deleted. {total} frame{} remain.",
                    if total == 1 { "" } else { "s" }
                );
            } else {
                state.status_message = "No frames to delete.".to_string();
            }
        }

        // ── Load selected frame's pipeline back into the effects editor ───────
        KeyCode::Enter => {
            if !state.animation.frames.is_empty() {
                state.load_animation_frame_pipeline();
                state.focused_panel = FocusedPanel::EffectsList;
                state.input_mode = InputMode::Normal;
                state.status_message =
                    "Frame pipeline loaded. Edit effects, then re-capture.".to_string();
            }
        }

        // ── Playback ──────────────────────────────────────────────────────────
        KeyCode::Char(' ') => {
            match &state.animation_playback {
                AnimationPlaybackState::Playing { current_frame, .. } => {
                    let cf = *current_frame;
                    state.animation_playback = AnimationPlaybackState::Paused { current_frame: cf };
                    state.status_message = "Playback paused.".to_string();
                }
                AnimationPlaybackState::Paused { current_frame } => {
                    let cf = *current_frame;
                    state.animation_playback = AnimationPlaybackState::Playing {
                        current_frame: cf,
                        frame_started: std::time::Instant::now(),
                    };
                    state.status_message = "Playback resumed.".to_string();
                }
                AnimationPlaybackState::Stopped => {
                    if state.animation.frames.len() < 2 {
                        state.status_message = "Add at least 2 frames before playing.".to_string();
                        return;
                    }
                    // Render any dirty frames before starting.
                    state.dispatch_render_dirty_frames();
                    let start = state.animation.selected;
                    state.animation_playback = AnimationPlaybackState::Playing {
                        current_frame: start,
                        frame_started: std::time::Instant::now(),
                    };
                    state.status_message = "Playback started.".to_string();
                }
            }
        }

        // ── Per-frame duration editing ─────────────────────────────────────────
        KeyCode::Char('f') => {
            if !state.animation.frames.is_empty() {
                let idx = state.animation.selected;
                let current_ms = state.animation.frames[idx].duration_ms;
                state.frame_duration_input = if current_ms == 0 {
                    state.animation.frame_duration_ms(idx).to_string()
                } else {
                    current_ms.to_string()
                };
                state.frame_duration_input_all = false;
                state.input_mode = InputMode::AnimationFrameDurationInput;
                state.status_message =
                    "Enter frame duration in ms (Enter to confirm, Esc to cancel):".to_string();
            }
        }

        // ── Set all frames to same duration ───────────────────────────────────
        KeyCode::Char('F') => {
            if !state.animation.frames.is_empty() {
                state.frame_duration_input = state.animation.frame_duration_ms(0).to_string();
                state.frame_duration_input_all = true;
                state.input_mode = InputMode::AnimationFrameDurationInput;
                state.status_message =
                    "Enter duration for ALL frames in ms (Enter to apply to all, Esc to cancel):"
                        .to_string();
            }
        }

        // ── Toggle loop mode ──────────────────────────────────────────────────
        KeyCode::Char('L') => {
            state.animation.loop_mode = !state.animation.loop_mode;
            state.status_message = if state.animation.loop_mode {
                "Loop mode enabled.".to_string()
            } else {
                "Loop mode disabled.".to_string()
            };
        }

        // ── Fps adjustment ────────────────────────────────────────────────────
        KeyCode::Char('+') | KeyCode::Char('=') => {
            state.animation.fps = (state.animation.fps + 1).min(60);
            state.status_message = format!("FPS: {}", state.animation.fps);
        }
        KeyCode::Char('-') => {
            state.animation.fps = state.animation.fps.saturating_sub(1).max(1);
            state.status_message = format!("FPS: {}", state.animation.fps);
        }

        // ── Frame reordering (mirrors effect reorder UX) ──────────────────────
        KeyCode::Char('K') | KeyCode::Up
            if modifiers.contains(KeyModifiers::SHIFT) || matches!(code, KeyCode::Char('K')) =>
        {
            let idx = state.animation.selected;
            if idx > 0 {
                state.animation.frames.swap(idx, idx - 1);
                state.animation_rendered_frames.swap(idx, idx - 1);
                state.animation.selected -= 1;
                state.status_message = "Frame moved up.".to_string();
            }
        }
        KeyCode::Char('J') | KeyCode::Down
            if modifiers.contains(KeyModifiers::SHIFT) || matches!(code, KeyCode::Char('J')) =>
        {
            let idx = state.animation.selected;
            let last = state.animation.frames.len().saturating_sub(1);
            if idx < last {
                state.animation.frames.swap(idx, idx + 1);
                state.animation_rendered_frames.swap(idx, idx + 1);
                state.animation.selected += 1;
                state.status_message = "Frame moved down.".to_string();
            }
        }

        // ── Parameter sweep dialog ────────────────────────────────────────────
        KeyCode::Char('s') => {
            if state.pipeline.effects.is_empty() {
                state.status_message =
                    "Add effects to the pipeline before using sweep.".to_string();
                return;
            }
            if state.source_asset.is_none() {
                state.status_message = "No image loaded — open an image first.".to_string();
                return;
            }
            // Pre-populate the sweep dialog with sensible defaults.
            let mut sw = SweepDialogState::default();
            sw.effect_idx = 0;
            sw.param_idx = 0;
            sw.frame_count = "12".to_string();
            // Pre-fill start/end from the first numeric param of the first effect.
            let descriptors = state.pipeline.effects[0].effect.param_descriptors();
            if let Some(d) = descriptors.first() {
                sw.start_value = crate::app::pipeline_utils::format_param_value(d.min);
                sw.end_value = crate::app::pipeline_utils::format_param_value(d.max);
            } else {
                sw.start_value = "0".to_string();
                sw.end_value = "1".to_string();
            }
            state.sweep_dialog = sw;
            state.input_mode = InputMode::AnimationSweepDialog;
            state.status_message =
                "Parameter sweep (↑↓: navigate, Enter: generate, Esc: cancel)".to_string();
        }

        // ── Animation export dialog ───────────────────────────────────────────
        KeyCode::Char('e') | KeyCode::Char('E') if modifiers.contains(KeyModifiers::CONTROL) => {
            open_animation_export_dialog(state);
        }

        // ── Return focus to effects panel ──────────────────────────────────────
        KeyCode::Esc | KeyCode::Tab => {
            state.input_mode = InputMode::Normal;
            state.focused_panel = FocusedPanel::EffectsList;
            state.status_message = "Animation panel unfocused.".to_string();
        }

        _ => {}
    }
}

/// Open the animation export dialog pre-populated with defaults.
fn open_animation_export_dialog(state: &mut AppState) {
    if state.animation.frames.len() < 2 {
        state.status_message = "Add at least 2 frames before exporting an animation.".to_string();
        return;
    }
    let default_filename = state
        .image_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .map(|s| format!("{}_anim", s.to_string_lossy()))
        .unwrap_or_else(|| "animation".to_string());
    state.animation_export_dialog.filename = default_filename;
    state.animation_export_dialog.focused_field = 1;
    state.input_mode = InputMode::AnimationExportDialog;
}

/// Show the pre-rendered image for frame `idx` in the canvas (best-effort).
fn show_animation_frame(state: &mut AppState, idx: usize) {
    if let Some(Some(img)) = state.animation_rendered_frames.get(idx) {
        let img = img.clone();
        state.set_preview(img);
    }
}

// ── Animation frame duration input ───────────────────────────────────────────

fn handle_animation_frame_duration_input(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            state.frame_duration_input.clear();
            state.frame_duration_input_all = false;
            state.input_mode = InputMode::AnimationPanel;
            state.status_message = "Duration edit cancelled.".to_string();
        }
        KeyCode::Enter => {
            let ms = state
                .frame_duration_input
                .parse::<u32>()
                .unwrap_or(0)
                .clamp(0, 60_000);

            if state.frame_duration_input_all {
                for f in &mut state.animation.frames {
                    f.duration_ms = ms;
                }
                state.status_message = format!("All frame durations set to {ms} ms.");
            } else {
                let idx = state.animation.selected;
                if let Some(f) = state.animation.frames.get_mut(idx) {
                    f.duration_ms = ms;
                }
                state.status_message = format!("Frame {} duration set to {ms} ms.", idx + 1);
            }
            state.frame_duration_input.clear();
            state.frame_duration_input_all = false;
            state.input_mode = InputMode::AnimationPanel;
        }
        KeyCode::Backspace => {
            state.frame_duration_input.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            state.frame_duration_input.push(c);
        }
        _ => {}
    }
}

// ── Animation sweep dialog ───────────────────────────────────────────────────

fn handle_animation_sweep_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_EFFECT: usize = 0;
    const FIELD_PARAM: usize = 1;
    const FIELD_START: usize = 2;
    const FIELD_END: usize = 3;
    const FIELD_FRAMES: usize = 4;
    const FIELD_EASING: usize = 5;
    const FIELD_COUNT: usize = 6;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::AnimationPanel;
            state.status_message = "Sweep cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.sweep_dialog.focused_field > 0 {
                state.sweep_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.sweep_dialog.focused_field < FIELD_COUNT - 1 {
                state.sweep_dialog.focused_field += 1;
            }
        }
        // Effect / Param / Easing cycling with Left/Right.
        KeyCode::Left | KeyCode::Right => {
            let f = state.sweep_dialog.focused_field;
            let n_effects = state.pipeline.effects.len();
            match f {
                FIELD_EFFECT if n_effects > 0 => {
                    let n = n_effects;
                    state.sweep_dialog.effect_idx = if matches!(code, KeyCode::Left) {
                        if state.sweep_dialog.effect_idx == 0 {
                            n - 1
                        } else {
                            state.sweep_dialog.effect_idx - 1
                        }
                    } else {
                        (state.sweep_dialog.effect_idx + 1) % n
                    };
                    state.sweep_dialog.param_idx = 0;
                    // Refresh start/end from the new effect's first param.
                    refresh_sweep_param_defaults(state);
                }
                FIELD_PARAM => {
                    let eff_idx = state.sweep_dialog.effect_idx;
                    let n = state.pipeline.effects[eff_idx]
                        .effect
                        .param_descriptors()
                        .len();
                    if n > 0 {
                        state.sweep_dialog.param_idx = if matches!(code, KeyCode::Left) {
                            if state.sweep_dialog.param_idx == 0 {
                                n - 1
                            } else {
                                state.sweep_dialog.param_idx - 1
                            }
                        } else {
                            (state.sweep_dialog.param_idx + 1) % n
                        };
                        refresh_sweep_param_defaults(state);
                    }
                }
                FIELD_EASING => {
                    let n = SWEEP_EASINGS.len();
                    state.sweep_dialog.easing_idx = if matches!(code, KeyCode::Left) {
                        if state.sweep_dialog.easing_idx == 0 {
                            n - 1
                        } else {
                            state.sweep_dialog.easing_idx - 1
                        }
                    } else {
                        (state.sweep_dialog.easing_idx + 1) % n
                    };
                }
                _ => {}
            }
        }
        KeyCode::Backspace => match state.sweep_dialog.focused_field {
            FIELD_START => {
                state.sweep_dialog.start_value.pop();
            }
            FIELD_END => {
                state.sweep_dialog.end_value.pop();
            }
            FIELD_FRAMES => {
                state.sweep_dialog.frame_count.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
            match state.sweep_dialog.focused_field {
                FIELD_START => state.sweep_dialog.start_value.push(c),
                FIELD_END => state.sweep_dialog.end_value.push(c),
                FIELD_FRAMES if c.is_ascii_digit() => state.sweep_dialog.frame_count.push(c),
                _ => {}
            }
        }
        KeyCode::Enter => {
            generate_sweep(state);
        }
        _ => {}
    }
}

/// Refresh the sweep dialog start/end from the selected effect + param defaults.
fn refresh_sweep_param_defaults(state: &mut AppState) {
    let eff_idx = state.sweep_dialog.effect_idx;
    let param_idx = state.sweep_dialog.param_idx;
    let descriptors = state.pipeline.effects[eff_idx].effect.param_descriptors();
    if let Some(d) = descriptors.get(param_idx) {
        state.sweep_dialog.start_value = crate::app::pipeline_utils::format_param_value(d.min);
        state.sweep_dialog.end_value = crate::app::pipeline_utils::format_param_value(d.max);
    }
}

/// Generate the sweep frames and dispatch batch rendering to the worker.
fn generate_sweep(state: &mut AppState) {
    let n_frames = state.sweep_dialog.parsed_frame_count();
    let start_val = state.sweep_dialog.parsed_start();
    let end_val = state.sweep_dialog.parsed_end();
    let easing = SWEEP_EASINGS[state.sweep_dialog.easing_idx].1;
    let effect_idx = state.sweep_dialog.effect_idx;
    let param_idx = state.sweep_dialog.param_idx;

    let base_pipeline = state.pipeline.clone();

    let mut pipelines: Vec<crate::effects::Pipeline> = Vec::with_capacity(n_frames);

    for i in 0..n_frames {
        let t_raw = if n_frames <= 1 {
            0.0f32
        } else {
            i as f32 / (n_frames - 1) as f32
        };
        let t = apply_easing(t_raw, easing);
        let value = start_val + (end_val - start_val) * t;

        let mut pipe = base_pipeline.clone();
        if effect_idx < pipe.effects.len() {
            let descriptors = pipe.effects[effect_idx].effect.param_descriptors();
            // Build the new params array, changing only param_idx.
            let mut values: Vec<f32> = descriptors.iter().map(|d| d.value).collect();
            if param_idx < values.len() {
                let d = &descriptors[param_idx];
                values[param_idx] = value.clamp(d.min, d.max);
            }
            let updated = pipe.effects[effect_idx].effect.apply_params(&values);
            pipe.effects[effect_idx].effect = updated;
        }
        pipelines.push(pipe);
    }

    let count = pipelines.len();
    state.dispatch_sweep_batch(pipelines);
    state.input_mode = InputMode::AnimationPanel;
    state.status_message = format!("Generating sweep: {count} frames… Worker rendering.");
}

// ── Animation export dialog ───────────────────────────────────────────────────

fn handle_animation_export_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_DIRECTORY: usize = 0;
    const FIELD_FILENAME: usize = 1;
    const FIELD_FORMAT: usize = 2;
    const FIELD_LOOP: usize = 3;
    const FIELD_COUNT: usize = 4;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::AnimationPanel;
            state.status_message = "Animation export cancelled.".to_string();
        }
        KeyCode::Enter => {
            dispatch_animation_export(state);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.animation_export_dialog.focused_field > 0 {
                state.animation_export_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.animation_export_dialog.focused_field < FIELD_COUNT - 1 {
                state.animation_export_dialog.focused_field += 1;
            }
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if state.animation_export_dialog.focused_field == FIELD_FORMAT =>
        {
            let n = ANIM_EXPORT_FORMATS.len();
            state.animation_export_dialog.format_index = match code {
                KeyCode::Left => {
                    if state.animation_export_dialog.format_index == 0 {
                        n - 1
                    } else {
                        state.animation_export_dialog.format_index - 1
                    }
                }
                _ => (state.animation_export_dialog.format_index + 1) % n,
            };
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if state.animation_export_dialog.focused_field == FIELD_LOOP =>
        {
            state.animation_export_dialog.loop_anim = !state.animation_export_dialog.loop_anim;
        }
        KeyCode::Backspace => match state.animation_export_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.animation_export_dialog.directory.pop();
            }
            FIELD_FILENAME => {
                state.animation_export_dialog.filename.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.animation_export_dialog.focused_field {
            FIELD_DIRECTORY => state.animation_export_dialog.directory.push(c),
            FIELD_FILENAME => state.animation_export_dialog.filename.push(c),
            _ => {}
        },
        _ => {}
    }
}

/// Build the export payload and dispatch it to the worker.
fn dispatch_animation_export(state: &mut AppState) {
    let total = state.animation.frames.len();
    if total < 2 {
        state.status_message = "Add at least 2 frames before exporting.".to_string();
        return;
    }
    // Collect all pre-rendered images. If some are missing, abort.
    let mut frames_payload: Vec<(image::DynamicImage, u32)> = Vec::with_capacity(total);
    for i in 0..total {
        match state.animation_rendered_frames.get(i) {
            Some(Some(img)) => {
                let duration_ms = state.animation.frame_duration_ms(i);
                frames_payload.push((img.clone(), duration_ms));
            }
            _ => {
                state.status_message =
                    format!("Frame {} is not yet rendered. Wait for worker.", i + 1);
                return;
            }
        }
    }

    let dialog = &state.animation_export_dialog;
    let ext = dialog.extension();
    let dir = std::path::PathBuf::from(&dialog.directory);
    let filename = dialog.effective_filename().to_string();
    let output_path = dir.join(format!("{filename}.{ext}"));
    let format_index = dialog.format_index;
    let loop_anim = dialog.loop_anim;

    let _ = state
        .worker_tx
        .send(crate::engine::worker::WorkerCommand::ExportAnimation {
            frames: frames_payload,
            output_path,
            format_index,
            loop_anim,
            response_tx: state.worker_resp_tx.clone(),
        });
    state.input_mode = InputMode::AnimationPanel;
    state.status_message = "Exporting animation…".to_string();
}
