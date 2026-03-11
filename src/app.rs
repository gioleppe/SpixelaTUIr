use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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

/// A single entry in the file browser – either a directory or an image file.
#[derive(Debug, Clone)]
pub enum FileBrowserEntry {
    Directory(std::path::PathBuf),
    /// Path and pre-fetched file size in bytes.
    ImageFile(std::path::PathBuf, u64),
}

/// State for the interactive file browser modal.
#[derive(Debug)]
pub struct FileBrowserState {
    /// Current working directory being browsed.
    pub cwd: std::path::PathBuf,
    /// Sorted list of entries: directories first, then image files.
    pub entries: Vec<FileBrowserEntry>,
    /// Currently highlighted row index.
    pub cursor: usize,
}

impl FileBrowserState {
    /// Supported image extensions.
    const IMAGE_EXTENSIONS: &'static [&'static str] =
        &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"];

    /// Create a new browser rooted at `dir`, reading its entries immediately.
    pub fn new(dir: std::path::PathBuf) -> Self {
        let mut state = Self {
            cwd: dir,
            entries: Vec::new(),
            cursor: 0,
        };
        state.refresh();
        state
    }

    /// Re-read the current directory, sorting dirs first then image files.
    pub fn refresh(&mut self) {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        let mut files: Vec<std::path::PathBuf> = Vec::new();

        if let Ok(read_dir) = std::fs::read_dir(&self.cwd) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase());
                    if let Some(e) = ext {
                        if Self::IMAGE_EXTENSIONS.contains(&e.as_str()) {
                            files.push(path);
                        }
                    }
                }
            }
        }

        dirs.sort();
        files.sort();

        self.entries = dirs
            .into_iter()
            .map(FileBrowserEntry::Directory)
            .chain(files.into_iter().map(|path| {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                FileBrowserEntry::ImageFile(path, size)
            }))
            .collect();
        self.cursor = 0;
    }

    /// Descend into the directory at `cursor`.
    pub fn enter_dir(&mut self) {
        if let Some(FileBrowserEntry::Directory(path)) = self.entries.get(self.cursor) {
            let new_dir = path.clone();
            self.cwd = new_dir;
            self.refresh();
        }
    }

    /// Ascend one level (go to parent directory).
    pub fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent().map(|p| p.to_path_buf()) {
            self.cwd = parent;
            self.refresh();
        }
    }

    /// Move the cursor up by one row.
    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor down by one row.
    pub fn move_down(&mut self) {
        if !self.entries.is_empty() && self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
        }
    }
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
    /// User is browsing the filesystem via the interactive file browser modal.
    FileBrowser,
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
    /// State for the interactive file-browser modal (Some when modal is open).
    pub file_browser: Option<FileBrowserState>,
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
            file_browser: None,
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

/// Keyboard hint shown in the controls bar and inside the file-browser footer.
pub const FILE_BROWSER_HINT: &str =
    "↑↓/jk: navigate  Enter: open  Backspace/-: up  Esc: cancel";

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
                    handle_key(&mut state, key.code);
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

fn handle_key(state: &mut AppState, code: KeyCode) {
    match state.input_mode {
        InputMode::Normal => handle_normal(state, code),
        InputMode::PathInput => handle_path_input(state, code),
        InputMode::AddEffect => handle_add_effect(state, code),
        InputMode::FileBrowser => handle_file_browser(state, code),
    }
}

fn handle_normal(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.should_quit = true;
        }
        KeyCode::Char('o') => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            state.file_browser = Some(FileBrowserState::new(cwd));
            state.input_mode = InputMode::FileBrowser;
        }
        KeyCode::Tab => {
            state.focused_panel = match state.focused_panel {
                FocusedPanel::Canvas => FocusedPanel::EffectsList,
                FocusedPanel::EffectsList => FocusedPanel::Canvas,
            };
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

fn handle_file_browser(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.file_browser = None;
            state.status_message = "Cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_up();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.move_down();
            }
        }
        KeyCode::Backspace | KeyCode::Char('-') => {
            if let Some(ref mut fb) = state.file_browser {
                fb.go_up();
            }
        }
        KeyCode::Enter => {
            // Clone what we need first to avoid the borrow-checker conflict.
            let action = state
                .file_browser
                .as_ref()
                .and_then(|fb| fb.entries.get(fb.cursor).cloned());
            match action {
                Some(FileBrowserEntry::Directory(_)) => {
                    if let Some(ref mut fb) = state.file_browser {
                        fb.enter_dir();
                    }
                }
                Some(FileBrowserEntry::ImageFile(path, _)) => {
                    state.input_mode = InputMode::Normal;
                    state.file_browser = None;
                    state.load_image(path);
                }
                None => {}
            }
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
