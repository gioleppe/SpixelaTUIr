use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{backend::Backend, Terminal};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::sync::mpsc;
use std::time::Duration;

use crate::effects::{
    Effect, Pipeline,
    color::ColorEffect,
    composite::CompositeEffect,
    crt::CrtEffect,
    glitch::GlitchEffect,
};
use crate::engine::worker::{WorkerCommand, WorkerResponse};

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Canvas,
    EffectsList,
}

/// Whether the application is accepting normal keyboard shortcuts or text input.
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    /// Normal shortcut mode (q, o, r, …).
    Normal,
    /// User is typing a file-system path for image loading.
    PathInput,
    /// User is browsing the add-effect menu.
    AddEffect,
}

/// Central application state
pub struct AppState {
    pub should_quit: bool,
    pub pipeline: Pipeline,
    pub image_path: Option<std::path::PathBuf>,

    // Worker thread communication
    pub worker_tx: mpsc::Sender<WorkerCommand>,
    pub worker_rx: mpsc::Receiver<WorkerResponse>,
    /// Clone of the response sender – given to the worker with each Process command.
    pub worker_resp_tx: mpsc::Sender<WorkerResponse>,

    // Image assets
    /// Full-resolution original image.
    pub source_asset: Option<image::DynamicImage>,
    /// Downscaled proxy used for live preview.
    pub proxy_asset: Option<image::DynamicImage>,
    /// Latest processed frame from the engine thread.
    pub preview_buffer: Option<image::DynamicImage>,

    // Rendering
    /// ratatui-image picker (detects terminal graphics capabilities).
    pub picker: Picker,
    /// ratatui-image stateful protocol for displaying the preview image.
    pub image_protocol: Option<StatefulProtocol>,

    // UI state
    pub status_message: String,
    pub input_mode: InputMode,
    pub focused_panel: FocusedPanel,
    /// Currently selected index in the active pipeline effect list.
    pub selected_effect: usize,
    /// Currently selected index in the add-effect menu.
    pub add_effect_cursor: usize,
    /// Buffer for the file-path typed by the user when in PathInput mode.
    pub path_input: String,
}

impl AppState {
    pub fn new(
        worker_tx: mpsc::Sender<WorkerCommand>,
        worker_rx: mpsc::Receiver<WorkerResponse>,
        worker_resp_tx: mpsc::Sender<WorkerResponse>,
        picker: Picker,
    ) -> Self {
        Self {
            should_quit: false,
            pipeline: Pipeline::default(),
            image_path: None,
            worker_tx,
            worker_rx,
            worker_resp_tx,
            source_asset: None,
            proxy_asset: None,
            preview_buffer: None,
            picker,
            image_protocol: None,
            status_message: "Press 'o' to open an image".to_string(),
            input_mode: InputMode::Normal,
            focused_panel: FocusedPanel::Canvas,
            selected_effect: 0,
            add_effect_cursor: 0,
            path_input: String::new(),
        }
    }

    /// Load an image from disk, create a proxy, and dispatch to the worker thread.
    pub fn load_image(&mut self, path: std::path::PathBuf) {
        match image::open(&path) {
            Ok(img) => {
                let proxy = img.thumbnail(800, 800);
                self.image_path = Some(path.clone());
                self.source_asset = Some(img);
                self.proxy_asset = Some(proxy);
                self.preview_buffer = None;
                self.image_protocol = None;
                self.dispatch_process();
                self.status_message = format!("Loading: {}", path.display());
            }
            Err(e) => {
                self.status_message = format!("Error opening image: {e}");
            }
        }
    }

    /// Send the current proxy image through the pipeline via the worker thread.
    pub fn dispatch_process(&self) {
        if let Some(path) = &self.image_path {
            let _ = self.worker_tx.send(WorkerCommand::Process {
                image_path: path.clone(),
                pipeline: self.pipeline.clone(),
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }

    /// Update the displayed image from a processed frame.
    pub fn set_preview(&mut self, img: image::DynamicImage) {
        self.image_protocol = Some(self.picker.new_resize_protocol(img.clone()));
        self.preview_buffer = Some(img);
    }

    /// Clamp the selected_effect cursor to valid bounds.
    pub fn clamp_selection(&mut self) {
        if self.pipeline.effects.is_empty() {
            self.selected_effect = 0;
        } else {
            self.selected_effect = self.selected_effect.min(self.pipeline.effects.len() - 1);
        }
    }

    /// Dispatch a PNG export of the current preview buffer.
    pub fn dispatch_export(&self) {
        if let Some(ref img) = self.preview_buffer {
            let path = self
                .image_path
                .as_ref()
                .map(|p| {
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                    std::path::PathBuf::from(format!("{stem}_out.png"))
                })
                .unwrap_or_else(|| std::path::PathBuf::from("output.png"));

            let _ = self.worker_tx.send(WorkerCommand::Export {
                image: img.clone(),
                output_path: path,
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }
}

/// All effects available to add, with display names.
pub const AVAILABLE_EFFECTS: &[(&str, fn() -> Effect)] = &[
    ("Invert",             || Effect::Color(ColorEffect::Invert)),
    ("HueShift +30°",      || Effect::Color(ColorEffect::HueShift { degrees: 30.0 })),
    ("Contrast ×1.5",      || Effect::Color(ColorEffect::Contrast { factor: 1.5 })),
    ("Saturation ×1.5",    || Effect::Color(ColorEffect::Saturation { factor: 1.5 })),
    ("Desaturate",         || Effect::Color(ColorEffect::Saturation { factor: 0.0 })),
    ("Quantize (4 levels)",|| Effect::Color(ColorEffect::ColorQuantization { levels: 4 })),
    ("Pixelate (8px)",     || Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })),
    ("Row Jitter",         || Effect::Glitch(GlitchEffect::RowJitter { magnitude: 0.05 })),
    ("Block Shift",        || Effect::Glitch(GlitchEffect::BlockShift { shift_x: 10, shift_y: 0 })),
    ("Pixel Sort",         || Effect::Glitch(GlitchEffect::PixelSort { threshold: 0.5 })),
    ("Scanlines",          || Effect::Crt(CrtEffect::Scanlines { spacing: 2, opacity: 0.5 })),
    ("Noise (RGB)",        || Effect::Crt(CrtEffect::Noise { intensity: 0.1, monochromatic: false })),
    ("Vignette",           || Effect::Crt(CrtEffect::Vignette { radius: 0.7, softness: 0.3 })),
    ("Crop 50%",           || Effect::Composite(CompositeEffect::CropRect { x: 50, y: 50, width: 200, height: 200 })),
];

/// Entry point for the application event loop.
pub fn run<B: Backend>(terminal: &mut Terminal<B>) -> Result<()>
where
    <B as Backend>::Error: Send + Sync + 'static,
{
    let (worker_tx, worker_cmd_rx) = mpsc::channel::<WorkerCommand>();
    let (worker_resp_tx, worker_rx) = mpsc::channel::<WorkerResponse>();

    let worker_handle = std::thread::spawn(move || {
        crate::engine::worker::run(worker_cmd_rx);
    });

    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let mut state = AppState::new(worker_tx, worker_rx, worker_resp_tx, picker);

    loop {
        while let Ok(response) = state.worker_rx.try_recv() {
            match response {
                WorkerResponse::ProcessedFrame(img) => {
                    let label = state
                        .image_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    state.set_preview(img);
                    state.status_message = format!("Ready | {label}");
                }
                WorkerResponse::Exported(path) => {
                    state.status_message = format!("Exported → {}", path.display());
                }
                WorkerResponse::Error(e) => {
                    state.status_message = format!("Engine error: {e}");
                }
            }
        }

        terminal.draw(|frame| {
            crate::ui::render(frame, &mut state);
        })?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    handle_key(&mut state, key.code, key.modifiers);
                }
            }
        }

        if state.should_quit {
            let _ = state.worker_tx.send(WorkerCommand::Quit);
            break;
        }
    }

    worker_handle.join().ok();
    Ok(())
}

fn handle_key(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    match state.input_mode {
        InputMode::Normal => handle_normal(state, code, modifiers),
        InputMode::PathInput => handle_path_input(state, code),
        InputMode::AddEffect => handle_add_effect(state, code),
    }
}

fn handle_normal(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
        }
        KeyCode::Char('o') => {
            state.input_mode = InputMode::PathInput;
            state.path_input.clear();
            state.status_message =
                "Enter image path (Enter to load, Esc to cancel):".to_string();
        }
        KeyCode::Tab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Canvas => FocusedPanel::EffectsList,
                FocusedPanel::EffectsList => FocusedPanel::Canvas,
            };
        }
        // Move selected effect one position up (Shift+Up or K).
        KeyCode::Up
            if state.focused_panel == FocusedPanel::EffectsList
                && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            move_effect_up(state);
        }
        KeyCode::Char('K') if state.focused_panel == FocusedPanel::EffectsList => {
            move_effect_up(state);
        }
        // Move selected effect one position down (Shift+Down or J).
        KeyCode::Down
            if state.focused_panel == FocusedPanel::EffectsList
                && modifiers.contains(KeyModifiers::SHIFT) =>
        {
            move_effect_down(state);
        }
        KeyCode::Char('J') if state.focused_panel == FocusedPanel::EffectsList => {
            move_effect_down(state);
        }
        // Effects-list navigation (when effects panel is focused).
        KeyCode::Up | KeyCode::Char('k')
            if state.focused_panel == FocusedPanel::EffectsList =>
        {
            if state.selected_effect > 0 {
                state.selected_effect -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j')
            if state.focused_panel == FocusedPanel::EffectsList =>
        {
            let max = state.pipeline.effects.len().saturating_sub(1);
            if state.selected_effect < max {
                state.selected_effect += 1;
            }
        }
        // Add effect.
        KeyCode::Char('a') if state.focused_panel == FocusedPanel::EffectsList => {
            state.input_mode = InputMode::AddEffect;
            state.add_effect_cursor = 0;
        }
        // Delete selected effect.
        KeyCode::Delete | KeyCode::Char('d')
            if state.focused_panel == FocusedPanel::EffectsList =>
        {
            if !state.pipeline.effects.is_empty() {
                state.pipeline.effects.remove(state.selected_effect);
                state.clamp_selection();
                state.status_message = "Effect removed. Re-processing…".to_string();
                state.image_protocol = None;
                state.dispatch_process();
            }
        }
        // Randomize effect parameters.
        KeyCode::Char('r') => {
            randomize_pipeline(&mut state.pipeline);
            state.status_message = "Randomised pipeline. Re-processing…".to_string();
            state.image_protocol = None;
            state.dispatch_process();
        }
        // Export current preview to PNG.
        KeyCode::Char('e') => {
            if state.preview_buffer.is_some() {
                state.dispatch_export();
                state.status_message = "Exporting…".to_string();
            } else {
                state.status_message = "No processed image to export.".to_string();
            }
        }
        _ => {}
    }
}

/// Move the selected effect one position up in the pipeline.
fn move_effect_up(state: &mut AppState) {
    let idx = state.selected_effect;
    if idx > 0 {
        state.pipeline.effects.swap(idx, idx - 1);
        state.selected_effect -= 1;
        state.status_message = "Moved effect up. Re-processing…".to_string();
        state.image_protocol = None;
        state.dispatch_process();
    }
}

/// Move the selected effect one position down in the pipeline.
fn move_effect_down(state: &mut AppState) {
    let idx = state.selected_effect;
    let last = state.pipeline.effects.len().saturating_sub(1);
    if idx < last {
        state.pipeline.effects.swap(idx, idx + 1);
        state.selected_effect += 1;
        state.status_message = "Moved effect down. Re-processing…".to_string();
        state.image_protocol = None;
        state.dispatch_process();
    }
}

fn handle_path_input(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.path_input.clear();
            state.status_message = "Cancelled.".to_string();
        }
        KeyCode::Enter => {
            let path = std::path::PathBuf::from(state.path_input.trim());
            state.input_mode = InputMode::Normal;
            state.path_input.clear();
            state.load_image(path);
        }
        KeyCode::Backspace => {
            state.path_input.pop();
        }
        KeyCode::Char(c) => {
            state.path_input.push(c);
        }
        _ => {}
    }
}

fn handle_add_effect(state: &mut AppState, code: KeyCode) {
    let n = AVAILABLE_EFFECTS.len();
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.add_effect_cursor > 0 { state.add_effect_cursor -= 1; }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.add_effect_cursor < n - 1 { state.add_effect_cursor += 1; }
        }
        KeyCode::Enter => {
            let effect = AVAILABLE_EFFECTS[state.add_effect_cursor].1();
            state.pipeline.effects.push(effect);
            state.input_mode = InputMode::Normal;
            state.selected_effect = state.pipeline.effects.len() - 1;
            state.status_message = format!(
                "Added '{}'. Re-processing…",
                AVAILABLE_EFFECTS[state.add_effect_cursor].0
            );
            state.image_protocol = None;
            state.dispatch_process();
        }
        _ => {}
    }
}

/// Randomize the numeric parameters of every effect in the pipeline.
fn randomize_pipeline(pipeline: &mut Pipeline) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    let seed = hasher.finish();

    // LCG: cheap deterministic random sequence from seed.
    let mut rng = seed;
    let mut next = move || -> f32 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) as f32) / (u32::MAX as f32)
    };

    for effect in &mut pipeline.effects {
        match effect {
            Effect::Color(e) => match e {
                ColorEffect::HueShift { degrees } => *degrees = next() * 360.0,
                ColorEffect::Contrast { factor } => *factor = 0.5 + next() * 2.5,
                ColorEffect::Saturation { factor } => *factor = next() * 2.0,
                ColorEffect::ColorQuantization { levels } => *levels = 2 + (next() * 6.0) as u8,
                ColorEffect::Invert => {}
            },
            Effect::Glitch(e) => match e {
                GlitchEffect::Pixelate { block_size } => *block_size = 2 + (next() * 20.0) as u32,
                GlitchEffect::RowJitter { magnitude } => *magnitude = next() * 0.2,
                GlitchEffect::BlockShift { shift_x, shift_y } => {
                    *shift_x = ((next() - 0.5) * 40.0) as i32;
                    *shift_y = ((next() - 0.5) * 40.0) as i32;
                }
                GlitchEffect::PixelSort { threshold } => *threshold = 0.2 + next() * 0.6,
            },
            Effect::Crt(e) => match e {
                CrtEffect::Scanlines { spacing, opacity } => {
                    *spacing = 2 + (next() * 4.0) as u32;
                    *opacity = 0.3 + next() * 0.7;
                }
                CrtEffect::Noise { intensity, .. } => *intensity = next() * 0.3,
                CrtEffect::Vignette { radius, softness } => {
                    *radius = 0.3 + next() * 0.5;
                    *softness = 0.1 + next() * 0.5;
                }
                CrtEffect::Curvature { strength } => *strength = next(),
                CrtEffect::PhosphorGlow { radius, intensity } => {
                    *radius = 1 + (next() * 5.0) as u32;
                    *intensity = next();
                }
            },
            Effect::Composite(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::{Effect, Pipeline, color::ColorEffect, glitch::GlitchEffect};
    use std::sync::mpsc;

    fn make_state_with_effects() -> AppState {
        let (worker_tx, _worker_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        let mut state = AppState::new(worker_tx, resp_rx, resp_tx, picker);
        state.pipeline = Pipeline {
            effects: vec![
                Effect::Color(ColorEffect::Invert),
                Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 }),
                Effect::Color(ColorEffect::HueShift { degrees: 30.0 }),
            ],
        };
        state.focused_panel = FocusedPanel::EffectsList;
        state.selected_effect = 1;
        state
    }

    #[test]
    fn move_effect_up_with_k() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('K'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        assert!(matches!(state.pipeline.effects[0], Effect::Glitch(_)));
        assert!(matches!(state.pipeline.effects[1], Effect::Color(ColorEffect::Invert)));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_up_with_shift_up() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 0);
        assert!(matches!(state.pipeline.effects[0], Effect::Glitch(_)));
        assert!(matches!(state.pipeline.effects[1], Effect::Color(ColorEffect::Invert)));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_down_with_j() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('J'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1], Effect::Color(_)));
        assert!(matches!(state.pipeline.effects[2], Effect::Glitch(_)));
        assert!(state.status_message.contains("down"));
    }

    #[test]
    fn move_effect_down_with_shift_down() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Down, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1], Effect::Color(_)));
        assert!(matches!(state.pipeline.effects[2], Effect::Glitch(_)));
        assert!(state.status_message.contains("down"));
    }

    #[test]
    fn move_effect_up_noop_at_first() {
        let mut state = make_state_with_effects();
        state.selected_effect = 0;
        let effects_before = state.pipeline.effects.clone();
        handle_normal(&mut state, KeyCode::Char('K'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        assert_eq!(state.pipeline.effects.len(), effects_before.len());
    }

    #[test]
    fn move_effect_down_noop_at_last() {
        let mut state = make_state_with_effects();
        state.selected_effect = 2;
        let effects_before = state.pipeline.effects.clone();
        handle_normal(&mut state, KeyCode::Char('J'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 2);
        assert_eq!(state.pipeline.effects.len(), effects_before.len());
    }

    #[test]
    fn plain_up_does_not_swap() {
        let mut state = make_state_with_effects();
        // Plain Up (no SHIFT) should only move cursor, not reorder effects.
        handle_normal(&mut state, KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 0);
        // First effect must remain the original Invert, not the Pixelate.
        assert!(matches!(state.pipeline.effects[0], Effect::Color(ColorEffect::Invert)));
    }
}
