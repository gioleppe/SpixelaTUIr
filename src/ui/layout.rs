use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Build the main layout.
///
/// Returns `[status_bar, canvas, effects_panel, controls]`.
///
/// ```
/// ┌─────────────────────────────────────┐
/// │ Status bar (1 line)                  │
/// ├──────────────────────────┬──────────┤
/// │                          │ Effects  │
/// │  Canvas                  │ Panel    │
/// │                          │ (24 cols)│
/// ├──────────────────────────┴──────────┤
/// │ Controls (3 lines)                   │
/// └─────────────────────────────────────┘
/// ```
pub fn build_layout(area: Rect) -> Vec<Rect> {
    // Outer vertical split: status | body | controls.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // status bar
            Constraint::Min(10),    // body
            Constraint::Length(3),  // controls
        ])
        .split(area);

    // Horizontal split inside body: canvas | effects panel.
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),        // canvas (takes remaining space)
            Constraint::Length(26),     // effects panel (fixed 26 columns)
        ])
        .split(outer[1]);

    vec![outer[0], body[0], body[1], outer[2]]
}
