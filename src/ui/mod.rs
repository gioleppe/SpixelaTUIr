pub mod canvas;
pub mod effects_panel;
pub mod file_browser;
pub mod layout;
pub mod widgets;

use ratatui::Frame;

use crate::app::AppState;

/// Top-level render function. Delegates to layout/widget modules.
pub fn render(frame: &mut Frame, state: &mut AppState) {
    // chunks = [status_bar, canvas, effects_panel, controls]
    let chunks = layout::build_layout(frame.area());
    widgets::render_status_bar(frame, chunks[0], state);
    canvas::render_canvas(frame, chunks[1], state);
    effects_panel::render_effects_panel(frame, chunks[2], state);
    widgets::render_controls(frame, chunks[3], state);
    // Overlays (rendered on top of everything).
    widgets::render_path_input(frame, state);
    effects_panel::render_add_effect_menu(frame, state);
    widgets::render_export_dialog(frame, state);
    widgets::render_save_pipeline_dialog(frame, state);
    widgets::render_help_modal(frame, state);
    file_browser::render_file_browser_modal(frame, state);
    effects_panel::render_edit_effect_modal(frame, state);
    widgets::render_quit_confirm_modal(frame, state);
}
