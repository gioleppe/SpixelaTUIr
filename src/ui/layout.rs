use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Height in terminal lines of the animation panel strip (including border).
pub const ANIMATION_PANEL_HEIGHT: u16 = 7;

/// Build the main layout.
///
/// Returns `[status_bar, canvas, effects_panel, controls_or_animation]`.
///
/// When `animation_open` is `false` the last element is the 3-line controls
/// hint (unchanged from the original layout).  When `animation_open` is `true`
/// the animation panel replaces the controls hint and expands to
/// [`ANIMATION_PANEL_HEIGHT`] lines; the canvas loses those extra lines.
///
/// ```
/// ┌─────────────────────────────────────┐
/// │ Status bar (1 line)                  │
/// ├──────────────────────────┬──────────┤
/// │                          │ Effects  │
/// │  Canvas                  │ Panel    │
/// │                          │ (24 cols)│
/// ├──────────────────────────┴──────────┤
/// │ Animation panel or Controls hint     │
/// └─────────────────────────────────────┘
/// ```
pub fn build_layout(area: Rect, animation_open: bool) -> Vec<Rect> {
    let bottom_height: u16 = if animation_open {
        ANIMATION_PANEL_HEIGHT
    } else {
        3 // controls hint panel height
    };

    // Outer vertical split: status | body | bottom.
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),             // status bar
            Constraint::Min(10),               // body
            Constraint::Length(bottom_height), // controls / animation
        ])
        .split(area);

    // Horizontal split inside body: canvas | effects panel.
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),    // canvas (takes remaining space)
            Constraint::Length(26), // effects panel (fixed 26 columns)
        ])
        .split(outer[1]);

    vec![outer[0], body[0], body[1], outer[2]]
}
