pub mod canvas;
pub mod layout;
pub mod widgets;

use ratatui::Frame;

use crate::app::AppState;

/// Top-level render function. Delegates to layout/widget modules.
pub fn render(frame: &mut Frame, state: &AppState) {
    let chunks = layout::build_layout(frame.area());
    widgets::render_status_bar(frame, chunks[0], state);
    canvas::render_canvas(frame, chunks[1], state);
    widgets::render_controls(frame, chunks[2], state);
}
