use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::{AppState, FILE_BROWSER_HINT, InputMode, PIPELINE_BROWSER_HINT};
use crate::engine::export::EXPORT_FORMATS;

/// Render the status bar at the top of the screen.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let res_label = if state.image_path.is_some() {
        let size = crate::app::PROXY_RESOLUTIONS[state.proxy_resolution_index];
        format!(" [{size}px]")
    } else {
        String::new()
    };

    // Animation indicator on the right side when the panel is open.
    let anim_label = if state.animation_panel_open {
        use crate::app::animation::AnimationPlaybackState;
        let n = state.animation.frames.len();
        let fps = state.animation.fps;
        let icon = match &state.animation_playback {
            AnimationPlaybackState::Playing { .. } => "▶",
            AnimationPlaybackState::Paused { .. } => "⏸",
            AnimationPlaybackState::Stopped => "■",
        };
        format!("   ANIM {n}F {fps}fps {icon}")
    } else {
        String::new()
    };

    let status = format!("Spix | {}{}{}", state.status_message, res_label, anim_label);
    let paragraph = Paragraph::new(status).style(
        Style::default()
            .fg(state.theme.text_normal)
            .bg(state.theme.inactive_border),
    );
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
        InputMode::SavePipelineDialog => "j/k: navigate fields  Enter: save as JSON  Esc: cancel",
        InputMode::HelpModal => "h / Esc: close help",
        InputMode::ConfirmClearPipeline => "Enter: confirm clear  Esc: cancel",
        InputMode::ConfirmQuit => "y / Enter: quit  n / Esc: cancel  s: save & stay",
        InputMode::Normal => {
            "q: Quit  o: Open  e: Export  Ctrl+S: Save  Ctrl+L: Load  Ctrl+Z/Y: Undo/Redo  Ctrl+D: Clear  Ctrl+N: Animation  r: Random  [/]: Preview  v: Split  H: Histogram  h: Help"
        }
        InputMode::AnimationPanel => {
            "c:capture  s:sweep  d:del  Space:play  Enter:edit-frame  f:dur  L:loop  +/-:fps  Ctrl+E:export  Esc:unfocus"
        }
        InputMode::AnimationSweepDialog => {
            "↑↓: field  ←/→: cycle  Type: value  Enter: generate  Esc: cancel"
        }
        InputMode::AnimationExportDialog => {
            "↑↓: field  ←/→: cycle  Type: value  Enter: export  Esc: cancel"
        }
        InputMode::AnimationFrameDurationInput => {
            "Type duration in ms  Enter: confirm  Esc: cancel"
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
        .border_style(Style::default().fg(state.theme.warning_border));
    let paragraph = Paragraph::new(display_text).block(block).style(
        Style::default()
            .fg(state.theme.text_normal)
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
        .border_style(Style::default().fg(state.theme.active_border));
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
                .fg(state.theme.selection_fg)
                .bg(state.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(state.theme.text_normal)
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
        Paragraph::new(preview).style(Style::default().fg(state.theme.text_dimmed)),
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
        .border_style(Style::default().fg(state.theme.success_border));
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
                .fg(state.theme.selection_fg)
                .bg(state.theme.success_border)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(state.theme.text_normal)
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
        Paragraph::new(preview).style(Style::default().fg(state.theme.text_dimmed)),
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
    // 46 lines: 15 global + 10 effects + 16 animation + borders/blank lines.
    let popup_height = total.height.min(46);
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
  Ctrl+D        Clear all effects (with confirm)\n\
  Ctrl+Z        Undo last pipeline change\n\
  Ctrl+Y        Redo last undone change\n\
  r             Randomise all effect parameters\n\
  [             Decrease preview resolution\n\
  ]             Increase preview resolution\n\
  v             Toggle side-by-side split view\n\
  H             Toggle live histogram overlay\n\
  Tab           Toggle focus: Canvas ↔ Effects ↔ Animation\n\
  h             Open / close this help\n\
  Ctrl+N        Toggle Animation panel\n\
\n\
 EFFECTS PANEL  (requires Effects panel focus)\n\
  ↑ / k         Navigate up\n\
  ↓ / j         Navigate down\n\
  a             Add a new effect\n\
  d / Delete    Delete selected effect\n\
  Enter         Edit effect parameters\n\
  Space         Toggle selected effect on/off\n\
  K / Shift+↑   Move effect up in pipeline\n\
  J / Shift+↓   Move effect down in pipeline\n\
\n\
 ANIMATION PANEL  (Tab to focus)\n\
  ← / →         Navigate frames\n\
  c             Capture current pipeline as frame\n\
  d / Delete    Delete selected frame\n\
  Enter         Load frame pipeline back for editing\n\
  Space         Play / pause\n\
  f             Set frame duration (ms)\n\
  F             Set ALL frames duration (ms)\n\
  L             Toggle loop mode\n\
  + / -         Increase / decrease fps\n\
  K / Shift+↑   Move frame up\n\
  J / Shift+↓   Move frame down\n\
  s             Parameter sweep dialog\n\
  Ctrl+E        Export animation (GIF/WebP)\n\
  Esc           Unfocus animation panel\n\
\n\
  Press h or Esc to close";

    let block = Block::default()
        .title(" Help – Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.warning_border));
    let paragraph = Paragraph::new(help_text).block(block);
    frame.render_widget(paragraph, popup_area);
}

/// Render the quit-confirmation modal when the user tries to quit with unsaved pipeline changes.
pub fn render_quit_confirm_modal(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::ConfirmQuit {
        return;
    }

    let total = frame.area();

    let popup_width = total.width.min(52);
    let popup_height = 7u16;
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let outer_block = Block::default()
        .title("  ⚠  Unsaved Changes  ")
        .borders(Borders::ALL)
        .border_style(
            Style::default()
                .fg(state.theme.error_border)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(outer_block, popup_area);

    let inner = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // blank
            Constraint::Length(1), // warning text
            Constraint::Length(1), // blank
            Constraint::Length(1), // key hints
            Constraint::Min(0),
        ])
        .split(inner);

    let warning = Paragraph::new("  You have unsaved pipeline changes.").style(
        Style::default()
            .fg(state.theme.warning_border)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(warning, rows[1]);

    let hints = Paragraph::new("  [y] Quit  [n] Cancel  [s] Save & stay")
        .style(Style::default().fg(state.theme.text_normal));
    frame.render_widget(hints, rows[3]);
}
