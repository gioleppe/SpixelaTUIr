use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::AppState;

/// Render the image canvas area.
///
/// If a processed preview frame exists, it is rendered via ratatui-image using
/// the best available terminal graphics protocol (Sixel → Kitty → half-blocks).
/// Otherwise a placeholder is shown.
pub fn render_canvas(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title("Canvas")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Inner area available for the image (inside the block borders).
    let inner = block.inner(area);

    // Render the block border first.
    frame.render_widget(block, area);

    if let Some(ref mut protocol) = state.image_protocol {
        // Render the image using ratatui-image (Sixel/half-blocks depending on terminal).
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, inner, protocol);
    } else {
        let msg = if state.image_path.is_some() {
            "Processing… please wait."
        } else {
            "No image loaded. Press 'o' to open a file."
        };
        let placeholder = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner);
    }
}
