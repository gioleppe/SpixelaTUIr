use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph,
        canvas::{Canvas, Line as CanvasLine},
    },
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::AppState;

/// Render the image canvas area.
///
/// Normal mode: the processed preview fills the entire canvas.  
/// Split-view mode (`state.split_view = true`): the canvas is divided
/// horizontally — left half shows the original proxy ("Before"), right half
/// shows the processed preview ("After").  
/// When `state.show_histogram` is true a compact luminance histogram is
/// overlaid in the top-right corner of the canvas (processed side).
pub fn render_canvas(frame: &mut Frame, area: Rect, state: &mut AppState) {
    if state.split_view {
        // In split view, render the halves directly into the provided area
        // to avoid double borders which can cause flickering with some Sixel terminals.
        render_split_canvas(frame, area, state);

        // Histogram overlay (top-right corner of canvas, processed side).
        if state.show_histogram {
            let halves = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);
            if let Some(ref img) = state.preview_buffer {
                let img_clone = img.clone();
                render_histogram_overlay(frame, halves[1], &img_clone, state);
            }
        }
    } else {
        let block = Block::default()
            .title("Canvas")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(state.theme.active_border));

        // Inner area available for the image (inside the block borders).
        let inner = block.inner(area);

        // Render the block border first.
        frame.render_widget(block, area);
        render_single_canvas(frame, inner, state);

        // Histogram overlay (top-right corner of canvas).
        if state.show_histogram
            && let Some(ref img) = state.preview_buffer
        {
            let img_clone = img.clone();
            render_histogram_overlay(frame, inner, &img_clone, state);
        }
    }
}

/// Render the full canvas with a single processed preview.
fn render_single_canvas(frame: &mut Frame, inner: Rect, state: &mut AppState) {
    if let Some(ref mut protocol) = state.image_protocol {
        log::debug!("Rendering processed preview in single-view mode");
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, inner, protocol);
        // Record the render area so `set_preview` can pre-encode the next
        // protocol replacement without triggering a Sixel blink.
        state.image_protocol_last_area = Some(inner);

        // Render direction overlay if needed
        render_direction_overlay(frame, inner, state);
    } else {
        let msg = if state.image_path.is_some() {
            "Processing… please wait."
        } else {
            "No image loaded. Press 'o' to open a file."
        };
        let placeholder = Paragraph::new(msg).style(Style::default().fg(state.theme.text_dimmed));
        frame.render_widget(placeholder, inner);
    }
}

/// Render the canvas split horizontally: Before (left) | After (right).
fn render_split_canvas(frame: &mut Frame, inner: Rect, state: &mut AppState) {
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    // ── Before (left) ─────────────────────────────────────────────────────
    let before_block = Block::default()
        .title("Before")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.text_dimmed));
    let before_inner = before_block.inner(halves[0]);
    frame.render_widget(before_block, halves[0]);

    if let Some(ref mut proto) = state.original_image_protocol {
        let widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(widget, before_inner, proto);
    } else {
        let msg = if state.image_path.is_some() {
            "Loading…"
        } else {
            "No image loaded."
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(state.theme.text_dimmed)),
            before_inner,
        );
    }

    // ── After (right) ──────────────────────────────────────────────────────
    let after_block = Block::default()
        .title("After")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.active_border));
    let after_inner = after_block.inner(halves[1]);
    frame.render_widget(after_block, halves[1]);

    if let Some(ref mut protocol) = state.image_protocol {
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, after_inner, protocol);
        // Record the render area so `set_preview` can pre-encode the next
        // protocol replacement without triggering a Sixel blink.
        state.image_protocol_last_area = Some(after_inner);

        // Render direction overlay if needed
        render_direction_overlay(frame, after_inner, state);
    } else {
        let msg = if state.image_path.is_some() {
            "Processing… please wait."
        } else {
            "No image loaded."
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(state.theme.text_dimmed)),
            after_inner,
        );
    }
}

// ── Histogram ────────────────────────────────────────────────────────────────

/// Number of histogram buckets (columns in the ASCII bar chart).
const HIST_BINS: usize = 32;
/// Height of the histogram widget in terminal rows (excluding border).
const HIST_HEIGHT: u16 = 6;
/// Width of the histogram widget in terminal columns (including border).
const HIST_WIDTH: u16 = (HIST_BINS as u16) + 2;

/// Render a compact luminance histogram as an ASCII bar-chart overlay.
///
/// The overlay is placed in the top-right corner of `canvas_inner`.
/// Uses only `preview_buffer` pixel data — no additional processing.
fn render_histogram_overlay(
    frame: &mut Frame,
    canvas_inner: Rect,
    img: &image::DynamicImage,
    state: &AppState,
) {
    // Compute the overlay rect (top-right corner).
    let overlay_h = HIST_HEIGHT + 2; // +2 for borders
    let overlay_w = HIST_WIDTH;
    if canvas_inner.width < overlay_w + 2 || canvas_inner.height < overlay_h + 1 {
        return; // Not enough space – skip silently.
    }
    let x = canvas_inner.x + canvas_inner.width - overlay_w;
    let y = canvas_inner.y;
    let overlay_area = Rect::new(x, y, overlay_w, overlay_h);

    // Build luminance histogram from the preview buffer.
    let mut bins = [0u64; HIST_BINS];
    let rgba = img.to_rgba8();
    for chunk in rgba.chunks(4) {
        let (r, g, b) = (chunk[0] as f32, chunk[1] as f32, chunk[2] as f32);
        // Rec.709 luminance.
        let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let bin = ((luma / 255.0) * (HIST_BINS as f32 - 1.0)) as usize;
        bins[bin.min(HIST_BINS - 1)] += 1;
    }

    // Normalise to HIST_HEIGHT rows.
    let max_count = *bins.iter().max().unwrap_or(&1).max(&1);
    let bar_heights: Vec<u8> = bins
        .iter()
        .map(|&c| ((c as f64 / max_count as f64) * HIST_HEIGHT as f64).round() as u8)
        .collect();

    // Build lines bottom-up: each terminal row represents one "level" of the bar.
    let mut lines: Vec<Line> = (0..HIST_HEIGHT)
        .rev()
        .map(|row| {
            let spans: Vec<Span> = bar_heights
                .iter()
                .map(|&h| {
                    // A bar of height `h` fills rows 0..h from the bottom.
                    // `row` is the current row index from bottom (0 = bottom-most).
                    let ch = if h > row as u8 { '█' } else { ' ' };
                    Span::styled(
                        ch.to_string(),
                        Style::default().fg(state.theme.success_border),
                    )
                })
                .collect();
            Line::from(spans)
        })
        .collect();
    // Append a scale row of dots at the bottom.
    lines.push(Line::from(
        (0..HIST_BINS)
            .map(|i| {
                let ch = if i == 0 || i == HIST_BINS / 2 || i == HIST_BINS - 1 {
                    '·'
                } else {
                    ' '
                };
                Span::styled(ch.to_string(), Style::default().fg(state.theme.text_dimmed))
            })
            .collect::<Vec<_>>(),
    ));

    let block = Block::default()
        .title("Luma")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state.theme.success_border));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
}

// ── Direction Overlay ────────────────────────────────────────────────────────

/// Renders a direction vector overlay on top of the image if the currently selected
/// effect in the pipeline has any parameter with `is_direction` set to `true`.
fn render_direction_overlay(frame: &mut Frame, area: Rect, state: &AppState) {
    if state.pipeline.effects.is_empty() {
        return;
    }

    let selected_idx = state.selected_effect;
    if selected_idx >= state.pipeline.effects.len() {
        return;
    }

    let effect = &state.pipeline.effects[selected_idx].effect;
    let params = effect.param_descriptors();

    // Check if there are any direction parameters
    for param in params {
        if param.is_direction {
            draw_vector_overlay(frame, area, param.value, state);
            // In case there are multiple direction parameters, we only draw one for now.
            // A more advanced approach could draw multiple vectors with different colors.
            break;
        }
    }
}

/// Draws a vector originating from the center of `area` and pointing towards `angle_degrees`.
fn draw_vector_overlay(frame: &mut Frame, area: Rect, angle_degrees: f32, state: &AppState) {
    use std::f32::consts::TAU;

    let angle_rad = angle_degrees * TAU / 360.0;

    // Calculate vector components
    let dir_x = angle_rad.cos();
    // In our effects, we typically consider image coordinates where Y goes down.
    // If angle is 90 degrees, in sine_warp we have dir_y = -sin(90) = -1, which means UP in image coords.
    // In ratatui canvas, Y goes UP by default. So if we want to point UP visually, Y must be positive.
    // So visual dir_y = -dir_y (from image coords) = sin(90) = 1.
    let visual_dir_y = angle_rad.sin();

    // The bounds of the canvas widget. We'll use -1.0 to 1.0.
    let canvas = Canvas::default()
        .block(Block::default())
        .x_bounds([-1.0, 1.0])
        .y_bounds([-1.0, 1.0])
        .paint(move |ctx| {
            // Draw a line from center (0,0) to the edge (dir_x, visual_dir_y)
            ctx.draw(&CanvasLine {
                x1: 0.0,
                y1: 0.0,
                x2: dir_x as f64,
                y2: visual_dir_y as f64,
                color: state.theme.active_border,
            });

            // Draw a small circle at the center
            ctx.draw(&ratatui::widgets::canvas::Points {
                coords: &[(0.0, 0.0)],
                color: state.theme.accent_1,
            });

            // Draw an arrowhead or end indicator
            ctx.draw(&ratatui::widgets::canvas::Points {
                coords: &[(dir_x as f64, visual_dir_y as f64)],
                color: state.theme.error_border,
            });
        });

    frame.render_widget(canvas, area);
}
