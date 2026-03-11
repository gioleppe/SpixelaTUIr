use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{AppState, AVAILABLE_EFFECTS, FocusedPanel, InputMode};
use crate::effects::{
    Effect,
    color::ColorEffect,
    composite::CompositeEffect,
    crt::CrtEffect,
    glitch::GlitchEffect,
};

/// Render the side-panel showing the active pipeline effects.
pub fn render_effects_panel(frame: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = state.focused_panel == FocusedPanel::EffectsList;
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if is_focused {
        "Effects [Tab]"
    } else {
        "Effects [Tab to focus]"
    };

    let block = Block::default()
        .title(title)
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
        .map(|(i, e)| {
            let selected = i == state.selected_effect && is_focused;
            let label = effect_label(e);
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let prefix = if selected { "▶ " } else { "  " };
            let suffix = if selected && is_focused && state.pipeline.effects.len() > 1 {
                " ↕"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
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
            CrtEffect::Noise { intensity, monochromatic } => {
                let kind = if *monochromatic { "mono" } else { "rgb" };
                format!("Noise {kind} {intensity:.2}")
            }
            CrtEffect::Vignette { radius, softness } => {
                format!("Vignette r={radius:.2} s={softness:.2}")
            }
        },
        Effect::Composite(c) => match c {
            CompositeEffect::ImageBlend { opacity } => format!("Blend {opacity:.0}%"),
            CompositeEffect::CropRect { x, y, width, height } => {
                format!("Crop {x},{y} {width}×{height}")
            }
        },
    }
}
