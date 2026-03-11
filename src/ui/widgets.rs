use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{AppState, InputMode, FILE_BROWSER_HINT};

/// Render the status bar at the top of the screen.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status = format!("SpixelaTUIr | {}", state.status_message);
    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

/// Render the controls panel (help text).
pub fn render_controls(frame: &mut Frame, area: Rect, state: &AppState) {
    let help = match state.input_mode {
        InputMode::PathInput => "Type path  Enter: load  Esc: cancel",
        InputMode::AddEffect => "j/k: navigate  Enter: add  Esc: cancel",
        InputMode::FileBrowser => FILE_BROWSER_HINT,
        InputMode::Normal => {
            "q: Quit  o: Open  e: Export  Tab: Switch panel  [Effects] a: Add  d: Del  r: Random"
        }
    };
    let block = Block::default().title("Controls").borders(Borders::ALL);
    let paragraph = Paragraph::new(help).block(block);
    frame.render_widget(paragraph, area);
}

/// Render the floating path-input overlay when the user presses 'o'.
pub fn render_path_input(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::PathInput {
        return;
    }

    let total = frame.area();

    // Centre a 70-wide, 3-tall popup.
    let popup_width = total.width.min(70);
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = total.height / 2 - 1;
    let popup_area = Rect::new(x, y, popup_width, 3);

    // Clear the area behind the popup so it appears floating.
    frame.render_widget(Clear, popup_area);

    let display_text = format!("{}_", state.path_input);
    let block = Block::default()
        .title("Open image (Enter to load, Esc to cancel)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let paragraph = Paragraph::new(display_text)
        .block(block)
        .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    frame.render_widget(paragraph, popup_area);
}
