use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;

/// Render the status bar at the top of the screen.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status = if let Some(ref path) = state.image_path {
        format!("SpixelaTUIr | {}", path.display())
    } else {
        "SpixelaTUIr | No image loaded".to_string()
    };
    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

/// Render the controls panel (sliders, menus, timeline strip).
pub fn render_controls(frame: &mut Frame, area: Rect, _state: &AppState) {
    let block = Block::default().title("Controls").borders(Borders::ALL);
    let help = Paragraph::new("q: Quit  o: Open image  r: Reset pipeline")
        .block(block);
    frame.render_widget(help, area);
}
