use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Build the main three-pane layout: status bar, canvas, controls.
pub fn build_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(area)
        .to_vec()
}
