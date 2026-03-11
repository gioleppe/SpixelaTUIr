use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{AppState, InputMode, FILE_BROWSER_HINT, PIPELINE_BROWSER_HINT};
use crate::engine::export::EXPORT_FORMATS;

/// Render the status bar at the top of the screen.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let status = format!("SpixelaTUIr | {}", state.status_message);
    let paragraph =
        Paragraph::new(status).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

/// Render the controls panel (help text).
pub fn render_controls(frame: &mut Frame, area: Rect, state: &AppState) {
    let help = match state.input_mode {
        InputMode::PathInput => "Type path  Enter: load  Esc: cancel",
        InputMode::AddEffect => "j/k: navigate  Enter: add  Esc: cancel",
        InputMode::FileBrowser => {
            use crate::app::FileBrowserPurpose;
            match state.file_browser.as_ref().map(|fb| &fb.purpose) {
                Some(FileBrowserPurpose::LoadPipeline) => PIPELINE_BROWSER_HINT,
                _ => FILE_BROWSER_HINT,
            }
        }
        InputMode::EditEffect { .. } => "j/k: next field  Type value  Enter: apply  Esc: cancel",
        InputMode::ExportDialog => {
            "j/k: navigate fields  ←/→/Space: cycle format  Enter: export  Esc: cancel"
        }
        InputMode::SavePipelineDialog => {
            "j/k: navigate fields  Enter: save as JSON  Esc: cancel"
        }
        InputMode::HelpModal => "h / Esc: close help",
        InputMode::Normal => {
            "q: Quit  o: Open  e: Export  Ctrl+S: Save  Ctrl+L: Load  r: Random  Tab: Switch focus  [/]: Preview  h: Help"
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
    let paragraph = Paragraph::new(display_text).block(block).style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(paragraph, popup_area);
}

/// Render the export dialog modal when the user presses 'e'.
pub fn render_export_dialog(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::ExportDialog {
        return;
    }

    let total = frame.area();

    // Centre a 60-wide, 10-tall popup.
    let popup_width = total.width.min(60);
    let popup_height = 10u16;
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let outer_block = Block::default()
        .title(" Export Image ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(outer_block, popup_area);

    // Inner area (inside the border).
    let inner = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    // Split inner into rows: Directory, Filename, Format, (blank), Preview.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Directory
            Constraint::Length(1), // Filename
            Constraint::Length(1), // Format
            Constraint::Length(1), // blank
            Constraint::Length(1), // Preview path
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    let dialog = &state.export_dialog;

    // Helper: style for a field row (highlighted when focused).
    let field_style = |idx: usize| {
        if dialog.focused_field == idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Directory row.
    let dir_cursor = if dialog.focused_field == 0 { "_" } else { "" };
    let dir_text = format!("  Directory  {}{}", dialog.directory, dir_cursor);
    frame.render_widget(Paragraph::new(dir_text).style(field_style(0)), rows[0]);

    // Filename row.
    let fn_cursor = if dialog.focused_field == 1 { "_" } else { "" };
    let fn_text = format!("  Filename   {}{}", dialog.filename, fn_cursor);
    frame.render_widget(Paragraph::new(fn_text).style(field_style(1)), rows[1]);

    // Format row.
    let format_name = EXPORT_FORMATS[dialog.format_index].display_name();
    let fmt_text = format!("  Format     [ {format_name} ]  (←/→ to change)");
    frame.render_widget(Paragraph::new(fmt_text).style(field_style(2)), rows[2]);

    // Preview path row.
    let ext = EXPORT_FORMATS[dialog.format_index].extension();
    let filename = dialog.effective_filename();
    let preview_path = std::path::Path::new(&dialog.directory)
        .join(format!("{filename}.{ext}"))
        .display()
        .to_string();
    let preview = format!("  Output:    {preview_path}");
    frame.render_widget(
        Paragraph::new(preview).style(Style::default().fg(Color::DarkGray)),
        rows[4],
    );
}

/// Render the save-pipeline dialog modal when the user presses Ctrl+S.
///
/// Mirrors the export-image dialog but has only Directory and Filename fields;
/// the output format is always JSON (enforced – no format selector shown).
pub fn render_save_pipeline_dialog(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::SavePipelineDialog {
        return;
    }

    let total = frame.area();

    // Centre a 60-wide, 9-tall popup.
    let popup_width = total.width.min(60);
    let popup_height = 9u16;
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let outer_block = Block::default()
        .title(" Save Pipeline (JSON) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    frame.render_widget(outer_block, popup_area);

    // Inner area (inside the border).
    let inner = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    // Rows: Directory, Filename, (blank), Preview.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Directory
            Constraint::Length(1), // Filename
            Constraint::Length(1), // blank
            Constraint::Length(1), // Preview path
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    let dialog = &state.save_pipeline_dialog;

    let field_style = |idx: usize| {
        if dialog.focused_field == idx {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Directory row.
    let dir_cursor = if dialog.focused_field == 0 { "_" } else { "" };
    let dir_text = format!("  Directory  {}{}", dialog.directory, dir_cursor);
    frame.render_widget(Paragraph::new(dir_text).style(field_style(0)), rows[0]);

    // Filename row (always appends .json).
    let fn_cursor = if dialog.focused_field == 1 { "_" } else { "" };
    let fn_text = format!("  Filename   {}{}.json", dialog.filename, fn_cursor);
    frame.render_widget(Paragraph::new(fn_text).style(field_style(1)), rows[1]);

    // Preview path row.
    let filename = dialog.effective_filename();
    let preview_path = std::path::Path::new(&dialog.directory)
        .join(format!("{filename}.json"))
        .display()
        .to_string();
    let preview = format!("  Output:    {preview_path}");
    frame.render_widget(
        Paragraph::new(preview).style(Style::default().fg(Color::DarkGray)),
        rows[3],
    );
}

/// Render the full keyboard-shortcut help overlay when the user presses 'h'.
pub fn render_help_modal(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::HelpModal {
        return;
    }

    let total = frame.area();

    let popup_width = total.width.min(62);
    let popup_height = total.height.min(28);
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let help_text = "\
 GLOBAL\n\
  q / Esc       Quit\n\
  o             Open image (file browser)\n\
  e             Export image (dialog)\n\
  Ctrl+S        Save pipeline (JSON dialog)\n\
  Ctrl+L        Load pipeline (file browser)\n\
  r             Randomise all effect parameters\n\
  [             Decrease preview resolution\n\
  ]             Increase preview resolution\n\
  Tab           Toggle focus: Canvas ↔ Effects\n\
  h             Open / close this help\n\
\n\
 EFFECTS PANEL  (requires Effects panel focus)\n\
  ↑ / k         Navigate up\n\
  ↓ / j         Navigate down\n\
  a             Add a new effect\n\
  d / Delete    Delete selected effect\n\
  Enter         Edit effect parameters\n\
  K / Shift+↑   Move effect up in pipeline\n\
  J / Shift+↓   Move effect down in pipeline\n\
\n\
  Press h or Esc to close";

    let block = Block::default()
        .title(" Help – Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let paragraph = Paragraph::new(help_text).block(block);
    frame.render_widget(paragraph, popup_area);
}
