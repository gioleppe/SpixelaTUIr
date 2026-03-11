use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::AppState;

/// Render the image canvas area.
///
/// If a processed preview frame exists, it is rendered via ratatui-image using
/// the best available terminal graphics protocol (Sixel → Kitty → half-blocks).
/// Otherwise a placeholder is shown.
///
/// When `state.show_histogram` is true a compact luminance/RGB histogram is
/// overlaid in the top-right corner of the inner canvas area.
pub fn render_canvas(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .title("Canvas")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Inner area available for the image (inside the block borders).
    let inner = block.inner(area);

    // Render the block border first.
    frame.render_widget(block, area);

    if let Some(ref mut protocol) = state.image_protocol {
        // Render the image using ratatui-image (Sixel/half-blocks depending on terminal).
        let image_widget = StatefulImage::default().resize(Resize::Fit(None));
        frame.render_stateful_widget(image_widget, inner, protocol);
    } else {
        let msg = if state.image_path.is_some() {
            "Processing… please wait."
        } else {
            "No image loaded. Press 'o' to open a file."
        };
        let placeholder = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner);
    }

    // Histogram overlay (top-right corner of inner canvas area).
    if state.show_histogram
        && let Some(ref img) = state.preview_buffer
    {
        render_histogram_overlay(frame, inner, img);
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
fn render_histogram_overlay(frame: &mut Frame, canvas_inner: Rect, img: &image::DynamicImage) {
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
    let bar_chars = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut lines: Vec<Line> = (0..HIST_HEIGHT)
        .rev()
        .map(|row| {
            let spans: Vec<Span> = bar_heights
                .iter()
                .map(|&h| {
                    // How many full rows this bar occupies above this row level.
                    let filled = h.saturating_sub(row as u8);
                    let ch = if filled == 0 {
                        ' '
                    } else if filled >= 1 {
                        bar_chars[8] // full block
                    } else {
                        ' '
                    };
                    Span::styled(
                        ch.to_string(),
                        Style::default().fg(Color::Green),
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
                Span::styled(ch.to_string(), Style::default().fg(Color::DarkGray))
            })
            .collect::<Vec<_>>(),
    ));

    let block = Block::default()
        .title("Luma")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, overlay_area);
}
