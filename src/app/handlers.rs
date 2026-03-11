use crossterm::event::{KeyCode, KeyModifiers};

use crate::effects::{Effect, EnabledEffect, color, color::ColorEffect};
use crate::engine::export::EXPORT_FORMATS;

use super::PROXY_RESOLUTIONS;
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
        }
        KeyCode::Tab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Canvas => FocusedPanel::EffectsList,
                FocusedPanel::EffectsList => FocusedPanel::Canvas,
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
        // Effects-list navigation (when effects panel is focused).
        KeyCode::Up | KeyCode::Char('k') if state.focused_panel == FocusedPanel::EffectsList => {
            if state.selected_effect > 0 {
                state.selected_effect -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') if state.focused_panel == FocusedPanel::EffectsList => {
            let max = state.pipeline.effects.len().saturating_sub(1);
            if state.selected_effect < max {
                state.selected_effect += 1;
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
        _ => {}
    }
}

// ── Effect reorder helpers ───────────────────────────────────────────────────

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
            if state.add_effect_cursor > 0 {
                state.add_effect_cursor -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.add_effect_cursor < n - 1 {
                state.add_effect_cursor += 1;
            }
        }
        KeyCode::Enter => {
            let effect = AVAILABLE_EFFECTS[state.add_effect_cursor].1();
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
            state.status_message = "Cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_down();
            }
        }
        KeyCode::Backspace | KeyCode::Char('-') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.go_up();
            }
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
                }
                Some(FileBrowserEntry::ImageFile(path, _)) => {
                    state.input_mode = InputMode::Normal;
                    state.file_browser = None;
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
            if field_idx > 0 {
                state.input_mode = InputMode::EditEffect {
                    field_idx: field_idx - 1,
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if field_idx + 1 < num_fields {
                state.input_mode = InputMode::EditEffect {
                    field_idx: field_idx + 1,
                };
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
