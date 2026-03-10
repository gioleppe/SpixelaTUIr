use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;

/// Render the image canvas area using ratatui-image for Sixel protocol support.
pub fn render_canvas(frame: &mut Frame, area: Rect, _state: &AppState) {
    let block = Block::default().title("Canvas").borders(Borders::ALL);
    let placeholder = Paragraph::new("No image loaded. Press 'o' to open a file.")
        .block(block);
    frame.render_widget(placeholder, area);
}
