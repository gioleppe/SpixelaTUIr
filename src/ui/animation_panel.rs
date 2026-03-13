//! Animation panel — frame strip, playback controls, and related modals.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::app::animation::{ANIM_EXPORT_FORMATS, AnimationPlaybackState, SWEEP_EASINGS};
use crate::app::{AppState, InputMode};

/// Render the animation panel strip at the bottom of the screen.
///
/// Panel height: 7 lines including border.
pub fn render_animation_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let focused = matches!(state.input_mode, InputMode::AnimationPanel)
        || matches!(state.input_mode, InputMode::AnimationFrameDurationInput);
    let border_color = if focused {
        state.theme.accent_1
    } else {
        state.theme.inactive_border
    };

    let n = state.animation.frames.len();
    let title = format!(" Animation ({n} frame{}) ", if n == 1 { "" } else { "s" });

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner into rows.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // frame strip
            Constraint::Length(1), // duration row
            Constraint::Length(1), // global controls / status
            Constraint::Min(0),    // padding
        ])
        .split(inner);

    render_frame_strip(frame, rows[0], state);
    render_duration_row(frame, rows[1], state);
    render_global_controls(frame, rows[2], state);
}

// ── Frame strip ───────────────────────────────────────────────────────────────

fn render_frame_strip(frame: &mut Frame, area: Rect, state: &AppState) {
    let n = state.animation.frames.len();
    if n == 0 {
        let hint =
            Paragraph::new("  No frames yet — focus this panel (Tab) and press 'c' to capture")
                .style(Style::default().fg(state.theme.text_dimmed));
        frame.render_widget(hint, area);
        return;
    }

    // Each slot: "[F01*]" = 6 chars. Reserve 2 chars for possible ◄/► indicators.
    let available_w = area.width.saturating_sub(2) as usize;
    let slots_visible = (available_w / 6).max(1);

    let sel = state.animation.selected;

    // Scroll so selected frame is always visible.
    let scroll_start = if sel >= slots_visible {
        sel - slots_visible + 1
    } else {
        0
    };
    let scroll_end = (scroll_start + slots_visible).min(n);

    let left_arrow = if scroll_start > 0 { "◄" } else { " " };
    let right_arrow = if scroll_end < n { "►" } else { " " };

    let mut strip = left_arrow.to_string();

    let playing_frame = state.animation_playback.current_frame();

    for i in scroll_start..scroll_end {
        let is_selected = i == sel;
        let is_rendered = state
            .animation_rendered_frames
            .get(i)
            .map_or(false, |f| f.is_some());
        let is_playing = playing_frame == Some(i);

        let marker = if is_playing {
            "▶"
        } else if is_selected {
            "*"
        } else {
            " "
        };

        let status = if !is_rendered { "?" } else { " " };
        strip.push_str(&format!("[F{:02}{marker}{status}]", i + 1));
    }

    strip.push_str(right_arrow);

    let strip_style = Style::default().fg(state.theme.text_normal);
    frame.render_widget(Paragraph::new(strip).style(strip_style), area);
}

// ── Duration row ──────────────────────────────────────────────────────────────

fn render_duration_row(frame: &mut Frame, area: Rect, state: &AppState) {
    let n = state.animation.frames.len();
    if n == 0 {
        return;
    }

    let available_w = area.width.saturating_sub(2) as usize;
    let slots_visible = (available_w / 6).max(1);
    let sel = state.animation.selected;
    let scroll_start = if sel >= slots_visible {
        sel - slots_visible + 1
    } else {
        0
    };
    let scroll_end = (scroll_start + slots_visible).min(n);

    let mut row = " ".to_string(); // align with left_arrow space

    for i in scroll_start..scroll_end {
        // Each slot is 6 chars wide; check before appending to prevent overflow.
        if row.len() + 7 >= area.width as usize {
            break;
        }
        let ms = state.animation.frame_duration_ms(i);
        row.push_str(&format!("{:>5}ms", ms));
    }

    frame.render_widget(
        Paragraph::new(row).style(Style::default().fg(state.theme.text_dimmed)),
        area,
    );
}

// ── Global controls ───────────────────────────────────────────────────────────

fn render_global_controls(frame: &mut Frame, area: Rect, state: &AppState) {
    let playback_icon = match &state.animation_playback {
        AnimationPlaybackState::Playing { current_frame, .. } => {
            format!("▶ F{:02}", current_frame + 1)
        }
        AnimationPlaybackState::Paused { current_frame } => {
            format!("⏸ F{:02}", current_frame + 1)
        }
        AnimationPlaybackState::Stopped => "■ Stopped".to_string(),
    };

    let loop_icon = if state.animation.loop_mode {
        "⟳"
    } else {
        "→"
    };
    let pending = if state.animation_pending_renders > 0 {
        format!("  [{} rendering…]", state.animation_pending_renders)
    } else {
        String::new()
    };

    let controls = format!(
        "  {playback_icon}  {loop_icon} {fps}fps{pending}",
        fps = state.animation.fps,
    );

    let hint = if matches!(state.input_mode, InputMode::AnimationPanel) {
        "  c:capture  s:sweep  d:del  Space:play  Enter:edit  f:dur  L:loop  +/-:fps  Ctrl+E:export  Esc:unfocus"
    } else {
        "  Tab to focus animation panel"
    };

    // Show controls on the left, hint on the right if space allows.
    let combined = format!("{controls}   {hint}");
    frame.render_widget(
        Paragraph::new(combined).style(Style::default().fg(state.theme.text_normal)),
        area,
    );
}

// ── Sweep dialog modal ─────────────────────────────────────────────────────────

pub fn render_sweep_dialog(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::AnimationSweepDialog {
        return;
    }

    let total = frame.area();
    let popup_width = total.width.min(60);
    let popup_height = 14u16;
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Parameter Sweep ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.accent_1));
    frame.render_widget(block, popup_area);

    let inner = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Effect
            Constraint::Length(1), // Parameter
            Constraint::Length(1), // Start
            Constraint::Length(1), // End
            Constraint::Length(1), // Frames
            Constraint::Length(1), // Easing
            Constraint::Length(1), // blank
            Constraint::Length(1), // hint
            Constraint::Min(0),
        ])
        .split(inner);

    let sw = &state.sweep_dialog;
    let field_style = |idx: usize| {
        if sw.focused_field == idx {
            Style::default()
                .fg(state.theme.selection_fg)
                .bg(state.theme.accent_1)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(state.theme.text_normal)
        }
    };

    // Effect row.
    let eff_name = state
        .pipeline
        .effects
        .get(sw.effect_idx)
        .map(|e| e.effect.variant_name())
        .unwrap_or("(none)");
    let eff_text = format!("  Effect      [{eff_name}]  (←/→)");
    frame.render_widget(Paragraph::new(eff_text).style(field_style(0)), rows[0]);

    // Parameter row.
    let param_name = state
        .pipeline
        .effects
        .get(sw.effect_idx)
        .and_then(|e| e.effect.param_descriptors().into_iter().nth(sw.param_idx))
        .map(|d| d.name)
        .unwrap_or("(none)");
    let param_text = format!("  Parameter   [{param_name}]  (←/→)");
    frame.render_widget(Paragraph::new(param_text).style(field_style(1)), rows[1]);

    // Start value.
    let start_cur = if sw.focused_field == 2 { "_" } else { "" };
    let start_text = format!("  Start       {}{start_cur}", sw.start_value);
    frame.render_widget(Paragraph::new(start_text).style(field_style(2)), rows[2]);

    // End value.
    let end_cur = if sw.focused_field == 3 { "_" } else { "" };
    let end_text = format!("  End         {}{end_cur}", sw.end_value);
    frame.render_widget(Paragraph::new(end_text).style(field_style(3)), rows[3]);

    // Frame count.
    let fc_cur = if sw.focused_field == 4 { "_" } else { "" };
    let fc_text = format!("  Frames      {}{fc_cur}", sw.frame_count);
    frame.render_widget(Paragraph::new(fc_text).style(field_style(4)), rows[4]);

    // Easing.
    let easing_name = SWEEP_EASINGS
        .get(sw.easing_idx)
        .map(|(name, _)| *name)
        .unwrap_or("Linear");
    let easing_text = format!("  Easing      [{easing_name}]  (←/→)");
    frame.render_widget(Paragraph::new(easing_text).style(field_style(5)), rows[5]);

    // Hint.
    let hint = Paragraph::new("  ↑↓: navigate  Enter: generate  Esc: cancel")
        .style(Style::default().fg(state.theme.text_dimmed));
    frame.render_widget(hint, rows[7]);
}

// ── Animation export dialog modal ─────────────────────────────────────────────

pub fn render_animation_export_dialog(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::AnimationExportDialog {
        return;
    }

    let total = frame.area();
    let popup_width = total.width.min(62);
    let popup_height = 12u16;
    let x = (total.width.saturating_sub(popup_width)) / 2;
    let y = (total.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Export Animation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.accent_1));
    frame.render_widget(block, popup_area);

    let inner = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Directory
            Constraint::Length(1), // Filename
            Constraint::Length(1), // Format
            Constraint::Length(1), // Loop
            Constraint::Length(1), // blank
            Constraint::Length(1), // output preview
            Constraint::Min(0),
        ])
        .split(inner);

    let dialog = &state.animation_export_dialog;

    let field_style = |idx: usize| {
        if dialog.focused_field == idx {
            Style::default()
                .fg(state.theme.selection_fg)
                .bg(state.theme.accent_1)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(state.theme.text_normal)
        }
    };

    // Directory.
    let dir_cur = if dialog.focused_field == 0 { "_" } else { "" };
    frame.render_widget(
        Paragraph::new(format!("  Directory  {}{}", dialog.directory, dir_cur))
            .style(field_style(0)),
        rows[0],
    );

    // Filename.
    let fn_cur = if dialog.focused_field == 1 { "_" } else { "" };
    frame.render_widget(
        Paragraph::new(format!("  Filename   {}{}", dialog.filename, fn_cur)).style(field_style(1)),
        rows[1],
    );

    // Format.
    let format_name = ANIM_EXPORT_FORMATS
        .get(dialog.format_index)
        .unwrap_or(&"GIF");
    frame.render_widget(
        Paragraph::new(format!("  Format     [ {format_name} ]  (←/→)")).style(field_style(2)),
        rows[2],
    );

    // Loop.
    let loop_val = if dialog.loop_anim { "Yes" } else { "No" };
    frame.render_widget(
        Paragraph::new(format!("  Loop       [ {loop_val} ]  (←/→ or Space)"))
            .style(field_style(3)),
        rows[3],
    );

    // Output preview.
    let ext = dialog.extension();
    let filename = dialog.effective_filename();
    let preview_path = std::path::Path::new(&dialog.directory)
        .join(format!("{filename}.{ext}"))
        .display()
        .to_string();
    frame.render_widget(
        Paragraph::new(format!("  Output:    {preview_path}"))
            .style(Style::default().fg(state.theme.text_dimmed)),
        rows[5],
    );
}
