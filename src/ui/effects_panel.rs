use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::app::{AVAILABLE_EFFECTS, AppState, FocusedPanel, InputMode};
use crate::effects::{
    Effect, color::ColorEffect, composite::CompositeEffect, crt::CrtEffect, glitch::GlitchEffect,
};

/// Render the side-panel showing the active pipeline effects.
pub fn render_effects_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = state.focused_panel == FocusedPanel::EffectsList;
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let effect_count = state.pipeline.effects.len();
    let title = format!("Effects ({effect_count})");

    let block = Block::default()
        .title(title.as_str())
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.pipeline.effects.is_empty() {
        let hint = if is_focused {
            "No effects.\nPress 'a' to add one."
        } else {
            "No effects.\nTab to focus."
        };
        let p = Paragraph::new(hint).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    let items: Vec<ListItem> = state
        .pipeline
        .effects
        .iter()
        .enumerate()
        .map(|(i, ee)| {
            let selected = i == state.selected_effect && is_focused;
            let label = effect_label(&ee.effect);
            let enabled = ee.enabled;
            let style = if selected && state.dragging_effect {
                // Distinct "dragging" highlight: cyan background.
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if !enabled {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if selected { "▶ " } else { "  " };
            let suffix = if selected && is_focused && state.pipeline.effects.len() > 1 {
                " ↕"
            } else {
                ""
            };
            let toggle_indicator = if enabled { "✓" } else { "✗" };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(toggle_indicator, style),
                Span::styled(label, style),
                Span::styled(suffix, style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    if is_focused {
        list_state.select(Some(state.selected_effect));
    }

    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/// Render the floating "add effect" menu overlay.
pub fn render_add_effect_menu(frame: &mut Frame, state: &AppState) {
    if state.input_mode != InputMode::AddEffect {
        return;
    }

    let total = frame.area();
    let popup_w = 34u16.min(total.width);
    let popup_h = (AVAILABLE_EFFECTS.len() as u16 + 2).min(total.height);
    let x = (total.width.saturating_sub(popup_w)) / 2;
    let y = (total.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title("Add Effect (Enter / Esc)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let items: Vec<ListItem> = AVAILABLE_EFFECTS
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let selected = i == state.add_effect_cursor;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if selected { "▶ " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(*name, style),
            ]))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(state.add_effect_cursor));

    let list = List::new(items);
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/// Render the floating "edit effect parameters" modal overlay.
pub fn render_edit_effect_modal(frame: &mut Frame, state: &AppState) {
    let field_idx = match state.input_mode {
        InputMode::EditEffect { field_idx } => field_idx,
        _ => return,
    };

    if state.pipeline.effects.is_empty() {
        return;
    }

    let effect = &state.pipeline.effects[state.selected_effect].effect;
    let descriptors = effect.param_descriptors();
    if descriptors.is_empty() {
        return;
    }

    let total = frame.area();
    // Width: 2 border cols + 2 indent + 14 name col + 2 spacing + "[ value_ ]" ≈ 44 chars.
    let popup_w = 44u16.min(total.width);
    // Height: one row per param + footer row + 2 border rows.
    let popup_h = (descriptors.len() as u16 + 3).min(total.height);
    let x = (total.width.saturating_sub(popup_w)) / 2;
    let y = (total.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let title = format!("Edit Effect: {}", effect_variant_name(effect));
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    // Split inner into [param rows, footer].
    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).split(inner);

    let items: Vec<ListItem> = descriptors
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let focused = i == field_idx;
            let value_str = state.edit_params.get(i).cloned().unwrap_or_default();
            let style = if focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let value_display = if focused {
                format!("[ {value_str}_ ]")
            } else {
                format!("  {value_str}  ")
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<14}", d.name), style),
                Span::styled(value_display, style),
            ]))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, chunks[0]);

    let footer =
        Paragraph::new("  Enter: apply   Esc: cancel").style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

/// Returns the human-readable variant name of an effect for use in the modal title.
///
/// Each variant maps to a `&'static str` matching its Rust identifier; e.g.
/// `Effect::Color(ColorEffect::HueShift { .. })` returns `"HueShift"`.
fn effect_variant_name(e: &Effect) -> &'static str {
    match e {
        Effect::Color(c) => match c {
            ColorEffect::Invert => "Invert",
            ColorEffect::HueShift { .. } => "HueShift",
            ColorEffect::Contrast { .. } => "Contrast",
            ColorEffect::Saturation { .. } => "Saturation",
            ColorEffect::ColorQuantization { .. } => "ColorQuantization",
        },
        Effect::Glitch(g) => match g {
            GlitchEffect::Pixelate { .. } => "Pixelate",
            GlitchEffect::RowJitter { .. } => "RowJitter",
            GlitchEffect::BlockShift { .. } => "BlockShift",
            GlitchEffect::PixelSort { .. } => "PixelSort",
        },
        Effect::Crt(c) => match c {
            CrtEffect::Scanlines { .. } => "Scanlines",
            CrtEffect::Curvature { .. } => "Curvature",
            CrtEffect::PhosphorGlow { .. } => "PhosphorGlow",
            CrtEffect::Noise { .. } => "Noise",
            CrtEffect::Vignette { .. } => "Vignette",
        },
        Effect::Composite(c) => match c {
            CompositeEffect::ImageBlend { .. } => "ImageBlend",
            CompositeEffect::CropRect { .. } => "CropRect",
        },
    }
}

/// Short human-readable label for an effect.
fn effect_label(e: &Effect) -> String {
    match e {
        Effect::Color(c) => match c {
            ColorEffect::Invert => "Invert".to_string(),
            ColorEffect::HueShift { degrees } => format!("HueShift {degrees:.0}°"),
            ColorEffect::Contrast { factor } => format!("Contrast ×{factor:.2}"),
            ColorEffect::Saturation { factor } => format!("Saturation ×{factor:.2}"),
            ColorEffect::ColorQuantization { levels } => format!("Quantize {levels}"),
        },
        Effect::Glitch(g) => match g {
            GlitchEffect::Pixelate { block_size } => format!("Pixelate {block_size}px"),
            GlitchEffect::RowJitter { magnitude } => format!("RowJitter {magnitude:.2}"),
            GlitchEffect::BlockShift { shift_x, shift_y } => {
                format!("BlockShift ({shift_x},{shift_y})")
            }
            GlitchEffect::PixelSort { threshold } => format!("PixelSort {threshold:.2}"),
        },
        Effect::Crt(c) => match c {
            CrtEffect::Scanlines { spacing, opacity } => {
                format!("Scanlines {spacing}px {opacity:.0}%")
            }
            CrtEffect::Curvature { strength } => format!("Curvature {strength:.2}"),
            CrtEffect::PhosphorGlow { radius, intensity } => {
                format!("PhosphorGlow r={radius} i={intensity:.2}")
            }
            CrtEffect::Noise {
                intensity,
                monochromatic,
            } => {
                let kind = if *monochromatic { "mono" } else { "rgb" };
                format!("Noise {kind} {intensity:.2}")
            }
            CrtEffect::Vignette { radius, softness } => {
                format!("Vignette r={radius:.2} s={softness:.2}")
            }
        },
        Effect::Composite(c) => match c {
            CompositeEffect::ImageBlend { opacity } => format!("Blend {opacity:.0}%"),
            CompositeEffect::CropRect {
                x,
                y,
                width,
                height,
            } => {
                format!("Crop {x},{y} {width}×{height}")
            }
        },
    }
}
