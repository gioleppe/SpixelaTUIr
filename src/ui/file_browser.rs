use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::app::{
    AppState, FILE_BROWSER_HINT, FileBrowserEntry, FileBrowserPurpose, InputMode,
    PIPELINE_BROWSER_HINT,
};

/// Render the floating file-browser modal over the whole terminal area.
pub fn render_file_browser_modal(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::FileBrowser {
        return;
    }

    let fb = match state.file_browser.as_ref() {
        Some(fb) => fb,
        None => return,
    };

    let total = frame.area();

    // Centre a popup that is 70% wide and 70% tall, with minimum dimensions.
    let popup_width = (total.width * 7 / 10).max(50).min(total.width);
    let popup_height = (total.height * 7 / 10).max(10).min(total.height);
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup.
    frame.render_widget(Clear, popup_area);

    // Outer border with title reflecting the browser's purpose.
    let title = match fb.purpose {
        FileBrowserPurpose::OpenImage => " Open Image ",
        FileBrowserPurpose::LoadPipeline => " Load Pipeline ",
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, popup_area);

    let inner_x = popup_area.x + 1;
    let inner_y = popup_area.y + 1;
    let inner_width = popup_area.width.saturating_sub(2);
    let inner_height = popup_area.height.saturating_sub(2);

    if inner_height == 0 || inner_width == 0 {
        return;
    }

    // Current directory path line.
    let path_text = fb.cwd.display().to_string();
    let path_area = Rect::new(inner_x, inner_y, inner_width, 1);
    let path_paragraph = Paragraph::new(path_text).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(path_paragraph, path_area);

    // We need at least path row + blank separator + list row + footer.
    if inner_height < 4 {
        return;
    }

    // File list area: leaves 1 row for path, 1 blank separator, 1 for footer.
    let list_height = inner_height.saturating_sub(3);
    let list_area = Rect::new(inner_x, inner_y + 2, inner_width, list_height);

    // Compute scroll offset so the cursor stays visible.
    let offset = scroll_offset(fb.cursor, list_height as usize);

    // Build list items.
    let items: Vec<ListItem> = fb
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(list_height as usize)
        .map(|(i, entry)| {
            let is_selected = i == fb.cursor;
            let (prefix, display_name, fg) = match entry {
                FileBrowserEntry::Directory(path) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    ("  ", format!("{name}/"), Color::Blue)
                }
                FileBrowserEntry::ImageFile(path, size) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    let size_str = format_size(*size);
                    let padded = format!(
                        "{:<width$} {:>8}",
                        name,
                        size_str,
                        width = (inner_width as usize).saturating_sub(12)
                    );
                    ("▶ ", padded, Color::White)
                }
            };

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(display_name, style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, list_area);

    // Footer hint.
    let footer_y = inner_y + inner_height - 1;
    let footer_area = Rect::new(inner_x, footer_y, inner_width, 1);
    let hint = match fb.purpose {
        FileBrowserPurpose::OpenImage => FILE_BROWSER_HINT,
        FileBrowserPurpose::LoadPipeline => PIPELINE_BROWSER_HINT,
    };
    let footer = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, footer_area);
}

/// Compute scroll offset so cursor remains in the visible window.
fn scroll_offset(cursor: usize, visible_height: usize) -> usize {
    if visible_height == 0 {
        return 0;
    }
    if cursor < visible_height {
        0
    } else {
        cursor - visible_height + 1
    }
}

/// Format a file size into a human-readable string (B / KB / MB).
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} MB", bytes / (1024 * 1024))
    }
}
