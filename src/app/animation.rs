//! Animation data model — frames, timeline, playback state, and dialog state.

use serde::{Deserialize, Serialize};

use crate::effects::Pipeline;

// ── Core animation data ──────────────────────────────────────────────────────

/// A single animation frame: a frozen pipeline snapshot + metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationFrame {
    /// The full pipeline state for this frame.
    pub pipeline: Pipeline,
    /// How long this frame is displayed during playback, in milliseconds.
    /// `0` means "use the timeline's global fps setting".
    pub duration_ms: u32,
    /// Optional label shown in the timeline strip (e.g. "Glitch peak").
    pub label: Option<String>,
}

/// The full animation timeline held in `AppState`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTimeline {
    pub frames: Vec<AnimationFrame>,
    /// Index of the currently selected frame in the timeline strip.
    pub selected: usize,
    /// Global frames-per-second used for uniform-duration playback (1–60).
    pub fps: u8,
    /// Whether export and playback should loop.
    pub loop_mode: bool,
}

impl Default for AnimationTimeline {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            selected: 0,
            fps: 10,
            loop_mode: true,
        }
    }
}

impl AnimationTimeline {
    /// Returns the effective display duration for a frame at `idx` in
    /// milliseconds. Uses the frame's own `duration_ms` if it is non-zero;
    /// otherwise falls back to `1000 / fps` (the globally uniform interval).
    /// `fps` is clamped to a minimum of 1 to avoid division by zero, which
    /// yields a maximum fallback duration of 1000 ms per frame.
    pub fn frame_duration_ms(&self, idx: usize) -> u32 {
        if idx < self.frames.len() {
            let d = self.frames[idx].duration_ms;
            if d > 0 {
                return d;
            }
        }
        let fps = self.fps.max(1) as u32;
        1000 / fps
    }

    /// Advance to the next frame, respecting loop mode.
    /// Returns `Some(next_idx)` or `None` if at end and not looping.
    pub fn next_frame(&self, current: usize) -> Option<usize> {
        let len = self.frames.len();
        if len == 0 {
            return None;
        }
        let next = current + 1;
        if next < len {
            Some(next)
        } else if self.loop_mode {
            Some(0)
        } else {
            None
        }
    }
}

// ── Playback state ───────────────────────────────────────────────────────────

/// Playback state for the animation live-preview in the canvas.
#[derive(Debug, Default)]
pub enum AnimationPlaybackState {
    #[default]
    Stopped,
    Playing {
        current_frame: usize,
        frame_started: std::time::Instant,
    },
    Paused {
        current_frame: usize,
    },
}

impl AnimationPlaybackState {
    /// Returns the current frame index if playing or paused.
    pub fn current_frame(&self) -> Option<usize> {
        match self {
            Self::Stopped => None,
            Self::Playing { current_frame, .. } | Self::Paused { current_frame } => {
                Some(*current_frame)
            }
        }
    }

    /// True while actively playing.
    pub fn is_playing(&self) -> bool {
        matches!(self, Self::Playing { .. })
    }
}

// ── Animation export dialog state ─────────────────────────────────────────────

/// Available animation export formats.
pub const ANIM_EXPORT_FORMATS: &[&str] = &["GIF", "WebP"];

/// State for the animation export dialog (Ctrl+E when animation panel focused).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationExportDialogState {
    /// Output directory (editable).
    pub directory: String,
    /// Base filename without extension (editable).
    pub filename: String,
    /// Index into `ANIM_EXPORT_FORMATS` (0=GIF, 1=WebP).
    pub format_index: usize,
    /// Whether the animation should loop in the exported file.
    pub loop_anim: bool,
    /// Which field has focus: 0=Directory, 1=Filename, 2=Format, 3=Loop.
    pub focused_field: usize,
}

impl Default for AnimationExportDialogState {
    fn default() -> Self {
        Self {
            directory: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .to_string_lossy()
                .into_owned(),
            filename: String::new(),
            format_index: 0,
            loop_anim: true,
            focused_field: 1,
        }
    }
}

impl AnimationExportDialogState {
    /// Return the effective filename, falling back to `"animation"` when blank.
    pub fn effective_filename(&self) -> &str {
        if self.filename.is_empty() {
            "animation"
        } else {
            &self.filename
        }
    }

    /// File extension for the selected format.
    pub fn extension(&self) -> &str {
        match self.format_index {
            1 => "webp",
            _ => "gif",
        }
    }
}

// ── Sweep dialog state ────────────────────────────────────────────────────────

/// Easing function for parameter sweep interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SweepEasing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Goes start→end in first half, end→start in second half — seamless loop.
    PingPong,
}

/// All available easing options with their display names.
pub const SWEEP_EASINGS: &[(&str, SweepEasing)] = &[
    ("Linear", SweepEasing::Linear),
    ("Ease-In", SweepEasing::EaseIn),
    ("Ease-Out", SweepEasing::EaseOut),
    ("Ease-In-Out", SweepEasing::EaseInOut),
    ("Ping-Pong", SweepEasing::PingPong),
];

/// Apply an easing function to a normalized time value `t` in `[0.0, 1.0]`.
pub fn apply_easing(t: f32, easing: SweepEasing) -> f32 {
    match easing {
        SweepEasing::Linear => t,
        SweepEasing::EaseIn => t * t,
        SweepEasing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
        SweepEasing::EaseInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0_f32 * t + 2.0_f32).powi(2) / 2.0
            }
        }
        // PingPong: the caller doubles frame count and this easing handles both halves.
        SweepEasing::PingPong => {
            if t <= 0.5 {
                t * 2.0
            } else {
                (1.0 - t) * 2.0
            }
        }
    }
}

/// State for the parameter sweep dialog (`s` key in animation panel).
///
/// Fields:
/// * 0 – Effect selector (↑/↓ to choose from current pipeline)
/// * 1 – Parameter selector (↑/↓ to choose numeric param of selected effect)
/// * 2 – Start value (text input)
/// * 3 – End value (text input)
/// * 4 – Frame count (text input)
/// * 5 – Easing (←/→ or ↑/↓ to cycle)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SweepDialogState {
    /// Index into the current pipeline effects list.
    pub effect_idx: usize,
    /// Index into the selected effect's `param_descriptors`.
    pub param_idx: usize,
    /// Start value as an editable string.
    pub start_value: String,
    /// End value as an editable string.
    pub end_value: String,
    /// Frame count as an editable string.
    pub frame_count: String,
    /// Index into `SWEEP_EASINGS`.
    pub easing_idx: usize,
    /// Which field currently has keyboard focus (0–5).
    pub focused_field: usize,
}

impl SweepDialogState {
    /// Parse `frame_count`, clamping to 2–240.  Falls back to 12.
    pub fn parsed_frame_count(&self) -> usize {
        self.frame_count
            .parse::<usize>()
            .unwrap_or(12)
            .clamp(2, 240)
    }

    /// Parse `start_value`.  Falls back to 0.0.
    pub fn parsed_start(&self) -> f32 {
        self.start_value.parse::<f32>().unwrap_or(0.0)
    }

    /// Parse `end_value`.  Falls back to 1.0.
    pub fn parsed_end(&self) -> f32 {
        self.end_value.parse::<f32>().unwrap_or(1.0)
    }
}
