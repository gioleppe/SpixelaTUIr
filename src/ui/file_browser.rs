use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::{
    AppState, FILE_BROWSER_HINT, FileBrowserEntry, FileBrowserPurpose, InputMode,
    PIPELINE_BROWSER_HINT,
};

/// Render the floating file-browser modal over the whole terminal area.
pub fn render_file_browser_modal(frame: &mut Frame, state: &mut AppState) {
    if state.input_mode != InputMode::FileBrowser {
        return;
    }

    let fb = match state.file_browser.as_ref() {
        Some(fb) => fb,
        None => return,
    };

    let total = frame.area();

    // For the OpenImage browser we widen the popup to accommodate the preview
    // pane; for pipeline loading we keep the compact layout.
    let is_open_image = fb.purpose == FileBrowserPurpose::OpenImage;

    let popup_width = if is_open_image {
        (total.width * 95 / 100).max(70).min(total.width)
    } else {
        (total.width * 7 / 10).max(50).min(total.width)
    };
    let popup_height = if is_open_image {
        (total.height * 90 / 100).max(10).min(total.height)
    } else {
        (total.height * 7 / 10).max(10).min(total.height)
    };
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

    if is_open_image {
        render_open_image_layout(frame, state, inner_x, inner_y, inner_width, inner_height);
    } else {
        render_pipeline_layout(frame, state, inner_x, inner_y, inner_width, inner_height);
    }
}

/// Layout for the "Open Image" browser: file list on the left, image preview
/// on the right.
fn render_open_image_layout(
    frame: &mut Frame,
    state: &mut AppState,
    inner_x: u16,
    inner_y: u16,
    inner_width: u16,
    inner_height: u16,
) {
    let fb = match state.file_browser.as_ref() {
        Some(fb) => fb,
        None => return,
    };

    // Split the inner area into list (left 35%) and preview (right 65%).
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(Rect::new(inner_x, inner_y, inner_width, inner_height));

    let list_area = columns[0];
    let preview_area = columns[1];

    // ── Left: path + file list + footer ─────────────────────────────────────
    let list_x = list_area.x;
    let list_y = list_area.y;
    let list_width = list_area.width;
    let list_height_total = list_area.height;

    // Current directory path line.
    let path_text = fb.cwd.display().to_string();
    let path_area = Rect::new(list_x, list_y, list_width, 1);
    let path_paragraph = Paragraph::new(path_text).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(path_paragraph, path_area);

    if list_height_total < 4 {
        return;
    }

    // File list occupies: total minus 1 path row, 1 blank separator, 1 footer.
    let file_list_height = list_height_total.saturating_sub(3);
    let file_list_area = Rect::new(list_x, list_y + 2, list_width, file_list_height);

    let offset = scroll_offset(fb.cursor, file_list_height as usize);
    let items = build_list_items(fb, offset, file_list_height as usize, list_width);

    let list = List::new(items);
    frame.render_widget(list, file_list_area);

    // Footer hint.
    let footer_y = list_y + list_height_total - 1;
    let footer_area = Rect::new(list_x, footer_y, list_width, 1);
    let footer = Paragraph::new(FILE_BROWSER_HINT).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, footer_area);

    // ── Right: image preview pane ────────────────────────────────────────────
    let preview_block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let preview_inner = preview_block.inner(preview_area);
    frame.render_widget(preview_block, preview_area);

    if let Some(ref mut protocol) = state.file_browser_preview {
        // Add a 1-cell margin on all sides so the image is centered with
        // breathing room inside the preview block.
        let padded = Rect::new(
            preview_inner.x + 1,
            preview_inner.y + 1,
            preview_inner.width.saturating_sub(2),
            preview_inner.height.saturating_sub(2),
        );
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, padded, protocol);
    } else {
        // Check whether the cursor is on a directory (no preview expected) or
        // an image file that is still loading.
        let is_image = state
            .file_browser
            .as_ref()
            .and_then(|fb| fb.entries.get(fb.cursor))
            .map(|e| matches!(e, FileBrowserEntry::ImageFile(..)))
            .unwrap_or(false);

        let msg = if is_image { "Loading preview…" } else { "No preview" };
        let placeholder =
            Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, preview_inner);
    }
}

/// Original compact layout used for pipeline loading (no preview pane).
fn render_pipeline_layout(
    frame: &mut Frame,
    state: &AppState,
    inner_x: u16,
    inner_y: u16,
    inner_width: u16,
    inner_height: u16,
) {
    let fb = match state.file_browser.as_ref() {
        Some(fb) => fb,
        None => return,
    };

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

    let items = build_list_items(fb, offset, list_height as usize, inner_width);
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

/// Build the list items for the visible window of the file browser.
fn build_list_items<'a>(
    fb: &crate::app::file_browser::FileBrowserState,
    offset: usize,
    visible: usize,
    column_width: u16,
) -> Vec<ListItem<'a>> {
    fb.entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible)
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
                        width = (column_width as usize).saturating_sub(12)
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
        .collect()
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
