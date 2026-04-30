use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::app::{AppState, FILE_BROWSER_HINT, InputMode, PIPELINE_BROWSER_HINT};
use crate::engine::export::EXPORT_FORMATS;

/// Render the status bar at the top of the screen.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &AppState) {
    let res_label = if state.image_path.is_some() {
        let size = state.proxy_resolutions[state.proxy_resolution_index];
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
        InputMode::AddEffect => "j/k/↑/↓:nav  Tab/←/→:tab  *:★Favs  f:★fav  Enter:add  Esc:close",
        InputMode::FileBrowser => {
            use crate::app::FileBrowserPurpose;
            match state.file_browser.as_ref().map(|fb| &fb.purpose) {
                Some(FileBrowserPurpose::LoadPipeline) => PIPELINE_BROWSER_HINT,
                _ => FILE_BROWSER_HINT,
            }
        }
        InputMode::EditEffect { .. } => "j/k: next field  Type value  Enter: apply  Esc: cancel",
        InputMode::ExportDialog => {
            "j/k: navigate fields  ←/→/Space: cycle  Type: edit  Enter: export  Esc: cancel"
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
    let paragraph = Paragraph::new(help).block(block).wrap(Wrap { trim: true });
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

    use crate::app::dialogs::ExportResolution;

    let total = frame.area();

    // Centre a 64-wide popup. Height grows by one row when the Custom
    // resolution edit field is shown so it doesn't overlap other rows.
    let custom_active = state.export_dialog.resolution == ExportResolution::Custom;
    let popup_width = total.width.min(64);
    let popup_height: u16 = if custom_active { 13 } else { 12 };
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

    // Split inner into rows: Directory, Filename, Format, Resolution,
    // (optional) Custom, blank, Source-dims hint, Preview path.
    let mut constraints: Vec<Constraint> = vec![
        Constraint::Length(1), // Directory
        Constraint::Length(1), // Filename
        Constraint::Length(1), // Format
        Constraint::Length(1), // Resolution
    ];
    if custom_active {
        constraints.push(Constraint::Length(1)); // Custom resolution input
    }
    constraints.extend_from_slice(&[
        Constraint::Length(1), // blank
        Constraint::Length(1), // Source-dims hint
        Constraint::Length(1), // Preview path
        Constraint::Min(0),    // padding
    ]);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
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
    let dir_text = format!("  Directory   {}{}", dialog.directory, dir_cursor);
    frame.render_widget(Paragraph::new(dir_text).style(field_style(0)), rows[0]);

    // Filename row.
    let fn_cursor = if dialog.focused_field == 1 { "_" } else { "" };
    let fn_text = format!("  Filename    {}{}", dialog.filename, fn_cursor);
    frame.render_widget(Paragraph::new(fn_text).style(field_style(1)), rows[1]);

    // Format row.
    let format_name = EXPORT_FORMATS[dialog.format_index].display_name();
    let fmt_text = format!("  Format      [ {format_name} ]  (←/→ to change)");
    frame.render_widget(Paragraph::new(fmt_text).style(field_style(2)), rows[2]);

    // Resolution row.
    let res_label = dialog.resolution.label();
    let res_text = format!("  Resolution  [ {res_label} ]  (←/→ to change)");
    frame.render_widget(Paragraph::new(res_text).style(field_style(3)), rows[3]);

    // Custom-resolution input row (only when Custom is selected).
    let mut row_idx = 4;
    if custom_active {
        let cursor = if dialog.focused_field == 4 { "_" } else { "" };
        let custom_text = format!("  Long edge   {}{}  px", dialog.custom_resolution, cursor);
        frame.render_widget(Paragraph::new(custom_text).style(field_style(4)), rows[4]);
        row_idx = 5;
    }

    // Blank row at row_idx; source-dims hint at row_idx + 1.
    let dims_row = row_idx + 1;
    let preview_row = row_idx + 2;

    // Source-dims hint.
    let dims_hint = match (&state.source_asset, dialog.resolution) {
        (Some(src), ExportResolution::Source) => format!(
            "  Source:    {}x{}  (saved at full resolution)",
            src.width(),
            src.height()
        ),
        (Some(src), ExportResolution::Preview) => {
            let (pw, ph) = state
                .preview_buffer
                .as_ref()
                .map(|p| (p.width(), p.height()))
                .unwrap_or((0, 0));
            format!(
                "  Source:    {}x{}   Preview: {}x{}",
                src.width(),
                src.height(),
                pw,
                ph
            )
        }
        (Some(src), ExportResolution::Custom) => {
            let long_edge = src.width().max(src.height());
            let target = dialog
                .custom_resolution_value()
                .map(|v| v.min(long_edge).to_string())
                .unwrap_or_else(|| "?".to_string());
            format!(
                "  Source:    {}x{}   Custom long edge: {} px (cap: {})",
                src.width(),
                src.height(),
                target,
                long_edge
            )
        }
        (None, _) => "  Source:    (none loaded)".to_string(),
    };
    frame.render_widget(
        Paragraph::new(dims_hint).style(Style::default().fg(state.theme.text_dimmed)),
        rows[dims_row],
    );

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
        rows[preview_row],
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

    // Two-column layout — target 90 wide × 30 tall so both columns are
    // well-balanced and the modal has good breadth on most terminals.
    let popup_width = total.width.min(90);
    let popup_height = total.height.min(30);
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let outer_block = Block::default()
        .title(" Help – Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.warning_border));
    let inner_area = outer_block.inner(popup_area);
    frame.render_widget(outer_block, popup_area);

    // Split the inner area into two equal columns.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    // ── Left column: GLOBAL + EFFECTS PANEL + ADD EFFECT MENU ───────────────
    let left_text = "\
 GLOBAL\n\
  q / Esc       Quit\n\
  o             Open image (file browser)\n\
  e             Export image (dialog)\n\
  Ctrl+S        Save pipeline\n\
  Ctrl+L        Load pipeline\n\
  Ctrl+D        Clear all effects\n\
  Ctrl+Z        Undo\n\
  Ctrl+Y        Redo\n\
  r             Randomise parameters\n\
  [ / ]         Decrease / increase preview\n\
  v             Toggle split view\n\
  H             Toggle histogram overlay\n\
  Tab           Cycle focus panel\n\
  Ctrl+N        Toggle Animation panel\n\
  h             Open / close this help\n\
\n\
 EFFECTS PANEL  (effects focus)\n\
  ↑ / k         Navigate up\n\
  ↓ / j         Navigate down\n\
  a             Add a new effect\n\
  *             Add effect → ★ Favs tab\n\
  d / Delete    Delete selected effect\n\
  Enter         Edit effect parameters\n\
  Space         Toggle effect on / off\n\
  K / Shift+↑   Move effect up\n\
  J / Shift+↓   Move effect down\n\
\n\
 ADD EFFECT MENU  (a or * to open)\n\
  ↑ / k         Navigate list up\n\
  ↓ / j         Navigate list down\n";

    // ── Right column: ANIMATION PANEL ────────────────────────────────────────
    let right_text = "\
 ADD EFFECT MENU (cont.)\n\
  Tab / →       Next category tab\n\
  Shift+Tab / ← Previous category tab\n\
  *             Jump to ★ Favs tab\n\
  f             Toggle ★ favorite\n\
  Enter         Add effect to pipeline\n\
  Esc           Close menu\n\
\n\
 ANIMATION PANEL  (Tab to focus)\n\
  ← / →         Navigate frames\n\
  c             Capture current as frame\n\
  d / Delete    Delete selected frame\n\
  Enter         Load frame pipeline\n\
  Space         Play / pause\n\
  f             Set frame duration (ms)\n\
  F             Set ALL frames duration\n\
  L             Toggle loop mode\n\
  + / -         Increase / decrease fps\n\
  K / Shift+↑   Move frame up\n\
  J / Shift+↓   Move frame down\n\
  s             Parameter sweep dialog\n\
  Ctrl+E        Export animation\n\
  Esc           Unfocus animation panel\n\
\n\
  Press h or Esc to close";

    frame.render_widget(Paragraph::new(left_text), cols[0]);
    frame.render_widget(Paragraph::new(right_text), cols[1]);
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
