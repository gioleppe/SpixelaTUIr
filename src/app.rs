use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{Terminal, backend::Backend};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::sync::mpsc;
use std::time::Duration;

use crate::effects::{
    Effect, EnabledEffect, Pipeline, color::ColorEffect, composite::CompositeEffect,
    crt::CrtEffect, glitch::GlitchEffect,
};
use crate::engine::export::{EXPORT_FORMATS, ExportFormat};
use crate::engine::worker::{WorkerCommand, WorkerResponse};

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Canvas,
    EffectsList,
}

/// A single entry in the file browser – either a directory or a selectable file.
#[derive(Debug, Clone)]
pub enum FileBrowserEntry {
    Directory(std::path::PathBuf),
    /// Path and pre-fetched file size in bytes.
    ImageFile(std::path::PathBuf, u64),
}

/// What the interactive file browser was opened for.
#[derive(Debug, Clone, PartialEq)]
pub enum FileBrowserPurpose {
    /// Selecting an image to load as the current source.
    OpenImage,
    /// Selecting a YAML / JSON pipeline file to import.
    LoadPipeline,
}

/// State for the interactive file browser modal.
#[derive(Debug)]
pub struct FileBrowserState {
    /// Current working directory being browsed.
    pub cwd: std::path::PathBuf,
    /// Sorted list of entries: directories first, then matching files.
    pub entries: Vec<FileBrowserEntry>,
    /// Currently highlighted row index.
    pub cursor: usize,
    /// Why the browser was opened (determines which file extensions are shown).
    pub purpose: FileBrowserPurpose,
}

impl FileBrowserState {
    /// Supported image extensions.
    const IMAGE_EXTENSIONS: &'static [&'static str] =
        &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "tif"];

    /// Supported pipeline file extensions.
    const PIPELINE_EXTENSIONS: &'static [&'static str] = &["yaml", "yml", "json"];

    /// Create a new browser rooted at `dir`, reading its entries immediately.
    pub fn new(dir: std::path::PathBuf, purpose: FileBrowserPurpose) -> Self {
        let mut state = Self {
            cwd: dir,
            entries: Vec::new(),
            cursor: 0,
            purpose,
        };
        state.refresh();
        state
    }

    /// File extensions accepted for the current purpose.
    fn accepted_extensions(&self) -> &'static [&'static str] {
        match self.purpose {
            FileBrowserPurpose::OpenImage => Self::IMAGE_EXTENSIONS,
            FileBrowserPurpose::LoadPipeline => Self::PIPELINE_EXTENSIONS,
        }
    }

    /// Re-read the current directory, sorting dirs first then matching files.
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
                    if let Some(e) = ext
                        && self.accepted_extensions().contains(&e.as_str())
                    {
                        files.push(path);
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
    /// User is editing parameters of the selected pipeline effect.
    EditEffect { field_idx: usize },
    /// User is configuring an export via the export dialog.
    ExportDialog,
    /// User is configuring a pipeline save via the save-pipeline dialog.
    SavePipelineDialog,
    /// User is viewing the full keyboard-shortcut help overlay.
    HelpModal,
    /// Waiting for the user to confirm clearing the pipeline (Ctrl+D).
    ConfirmClearPipeline,
}

/// State for the export dialog modal.
#[derive(Debug, Clone)]
pub struct ExportDialogState {
    /// Output directory (editable).
    pub directory: String,
    /// Base filename without extension (editable).
    pub filename: String,
    /// Index into `EXPORT_FORMATS`.
    pub format_index: usize,
    /// Which field has focus: 0 = Directory, 1 = Filename, 2 = Format.
    pub focused_field: usize,
}

impl ExportDialogState {
    /// Return the effective filename, falling back to `"output"` when the field is blank.
    pub fn effective_filename(&self) -> &str {
        if self.filename.is_empty() {
            "output"
        } else {
            &self.filename
        }
    }
}

/// State for the save-pipeline dialog modal (mirrors [`ExportDialogState`] but enforces JSON).
#[derive(Debug, Clone)]
pub struct SavePipelineDialogState {
    /// Output directory (editable).
    pub directory: String,
    /// Base filename without extension (editable). The `.json` extension is appended automatically.
    pub filename: String,
    /// Which field has focus: 0 = Directory, 1 = Filename.
    pub focused_field: usize,
}

impl SavePipelineDialogState {
    /// Return the effective filename, falling back to `"pipeline"` when the field is blank.
    pub fn effective_filename(&self) -> &str {
        if self.filename.is_empty() {
            "pipeline"
        } else {
            &self.filename
        }
    }
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
    /// Per-field string buffers used while editing effect parameters.
    pub edit_params: Vec<String>,
    /// State for the export dialog when in ExportDialog mode.
    pub export_dialog: ExportDialogState,
    /// State for the save-pipeline dialog when in SavePipelineDialog mode.
    pub save_pipeline_dialog: SavePipelineDialogState,
    /// Index into `PROXY_RESOLUTIONS` – controls live-preview quality.
    pub proxy_resolution_index: usize,
    /// True while the user is actively moving an effect with K / J (drag-to-reorder).
    pub dragging_effect: bool,
    /// Set whenever the pipeline is modified; cleared when the pipeline is saved.
    pub pipeline_dirty: bool,
    /// True after the first quit attempt when there are unsaved changes (double-press to confirm).
    pub quit_requested: bool,
    /// Ring-buffer of past pipeline states for undo (most-recent-first).
    pub undo_stack: std::collections::VecDeque<Pipeline>,
    /// Stack of pipeline states that were undone, for redo.
    pub redo_stack: std::collections::VecDeque<Pipeline>,
    /// Whether to show the live luminance/RGB histogram overlay on the canvas.
    pub show_histogram: bool,
    /// Whether the canvas is in side-by-side before/after split view.
    pub split_view: bool,
    /// ratatui-image stateful protocol for displaying the original (pre-effects) proxy.
    /// Only populated when `split_view` is active and an image is loaded.
    pub original_image_protocol: Option<StatefulProtocol>,
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
            edit_params: Vec::new(),
            export_dialog: ExportDialogState {
                directory: std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .into_owned(),
                filename: String::new(),
                format_index: 0,
                focused_field: 1,
            },
            save_pipeline_dialog: SavePipelineDialogState {
                directory: std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .into_owned(),
                filename: String::new(),
                focused_field: 1,
            },
            // Default to index 1 (512 px) — preserves prior behaviour.
            proxy_resolution_index: 1,
            dragging_effect: false,
            pipeline_dirty: false,
            quit_requested: false,
            undo_stack: std::collections::VecDeque::new(),
            redo_stack: std::collections::VecDeque::new(),
            show_histogram: false,
            split_view: false,
            original_image_protocol: None,
        }
    }

    /// Load an image from disk, create a proxy, and dispatch to the worker thread.
    pub fn load_image(&mut self, path: std::path::PathBuf) {
        match image::open(&path) {
            Ok(img) => {
                let size = PROXY_RESOLUTIONS[self.proxy_resolution_index];
                let proxy = img.thumbnail(size, size);
                self.image_path = Some(path.clone());
                self.source_asset = Some(img);
                self.original_image_protocol = Some(self.picker.new_resize_protocol(proxy.clone()));
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
    ///
    /// The proxy (pre-decoded, downscaled `DynamicImage`) is cloned directly
    /// into the command, so the worker never reads from disk during editing.
    pub fn dispatch_process(&self) {
        if let Some(proxy) = &self.proxy_asset {
            let _ = self.worker_tx.send(WorkerCommand::Process {
                image: proxy.clone(),
                pipeline: self.pipeline.clone(),
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }

    /// Re-scale the proxy from `source_asset` at the current resolution tier and
    /// re-dispatch the pipeline.  Calls from keyboard shortcuts `[` / `]`.
    pub fn reload_proxy(&mut self) {
        if let Some(source) = self.source_asset.take() {
            let size = PROXY_RESOLUTIONS[self.proxy_resolution_index];
            let proxy = source.thumbnail(size, size);
            self.source_asset = Some(source);
            self.original_image_protocol =
                Some(self.picker.new_resize_protocol(proxy.clone()));
            self.proxy_asset = Some(proxy);
            self.preview_buffer = None;
            self.image_protocol = None;
            self.dispatch_process();
            self.status_message = format!("Preview resolution: {size}px — Re-processing…");
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

    /// Dispatch an export of the current preview buffer with the given path and format.
    pub fn dispatch_export(&self, output_path: std::path::PathBuf, format: ExportFormat) {
        if let Some(ref img) = self.preview_buffer {
            let _ = self.worker_tx.send(WorkerCommand::Export {
                image: img.clone(),
                output_path,
                format,
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }

    /// Push the current pipeline onto the undo stack before a mutation, clearing redo.
    ///
    /// The stack is capped at 20 entries; the oldest entry is dropped when the
    /// capacity is exceeded.
    pub fn push_undo(&mut self) {
        const MAX_UNDO: usize = 20;
        self.undo_stack.push_front(self.pipeline.clone());
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.pop_back();
        }
        self.redo_stack.clear();
    }
}

/// Keyboard hint shown in the controls bar and inside the file-browser footer.
pub const FILE_BROWSER_HINT: &str = "↑↓/jk: navigate  Enter: open  Backspace/-: up  Esc: cancel";

/// Available proxy resolution tiers (max pixels on the long edge).
/// Index 1 (512 px) is the default — matches the previous hardcoded value.
pub const PROXY_RESOLUTIONS: &[u32] = &[256, 512, 768, 1024];
/// Keyboard hint shown when the file browser is open to load a pipeline.
pub const PIPELINE_BROWSER_HINT: &str =
    "↑↓/jk: navigate  Enter: load  Backspace/-: up  Esc: cancel";

/// All effects available to add, with display names.
pub type EffectEntry = (&'static str, fn() -> Effect);
pub const AVAILABLE_EFFECTS: &[EffectEntry] = &[
    ("Invert", || Effect::Color(ColorEffect::Invert)),
    ("HueShift +30°", || {
        Effect::Color(ColorEffect::HueShift { degrees: 30.0 })
    }),
    ("Contrast ×1.5", || {
        Effect::Color(ColorEffect::Contrast { factor: 1.5 })
    }),
    ("Saturation ×1.5", || {
        Effect::Color(ColorEffect::Saturation { factor: 1.5 })
    }),
    ("Desaturate", || {
        Effect::Color(ColorEffect::Saturation { factor: 0.0 })
    }),
    ("Quantize (4 levels)", || {
        Effect::Color(ColorEffect::ColorQuantization { levels: 4 })
    }),
    ("Pixelate (8px)", || {
        Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })
    }),
    ("Row Jitter", || {
        Effect::Glitch(GlitchEffect::RowJitter { magnitude: 0.05 })
    }),
    ("Block Shift", || {
        Effect::Glitch(GlitchEffect::BlockShift {
            shift_x: 10,
            shift_y: 0,
        })
    }),
    ("Pixel Sort", || {
        Effect::Glitch(GlitchEffect::PixelSort { threshold: 0.5 })
    }),
    ("Scanlines", || {
        Effect::Crt(CrtEffect::Scanlines {
            spacing: 2,
            opacity: 0.5,
        })
    }),
    ("Noise (RGB)", || {
        Effect::Crt(CrtEffect::Noise {
            intensity: 0.1,
            monochromatic: false,
        })
    }),
    ("Vignette", || {
        Effect::Crt(CrtEffect::Vignette {
            radius: 0.7,
            softness: 0.3,
        })
    }),
    ("Crop 50%", || {
        Effect::Composite(CompositeEffect::CropRect {
            x: 50,
            y: 50,
            width: 200,
            height: 200,
        })
    }),
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

        if event::poll(Duration::from_millis(16))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            handle_key(&mut state, key.code, key.modifiers);
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
        InputMode::FileBrowser => handle_file_browser(state, code),
        InputMode::EditEffect { .. } => handle_edit_effect(state, code),
        InputMode::ExportDialog => handle_export_dialog(state, code),
        InputMode::SavePipelineDialog => handle_save_pipeline_dialog(state, code),
        InputMode::HelpModal => handle_help_modal(state, code),
        InputMode::ConfirmClearPipeline => handle_confirm_clear_pipeline(state, code),
    }
}

fn handle_normal(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    // Any keypress ends the drag highlight by default; move_effect_* will re-enable it.
    state.dragging_effect = false;
    // Any non-quit keypress resets the pending-quit confirmation.
    if !matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
        state.quit_requested = false;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if state.pipeline_dirty && !state.quit_requested {
                state.quit_requested = true;
                state.status_message =
                    "Unsaved changes – press q again to quit, or Ctrl+S to save.".to_string();
            } else {
                state.should_quit = true;
            }
        }
        KeyCode::Char('o') => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            state.file_browser = Some(FileBrowserState::new(cwd, FileBrowserPurpose::OpenImage));
            state.input_mode = InputMode::FileBrowser;
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
        KeyCode::Up | KeyCode::Char('k') if state.focused_panel == FocusedPanel::EffectsList => {
            if state.selected_effect > 0 {
                state.selected_effect -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') if state.focused_panel == FocusedPanel::EffectsList => {
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
        // Toggle the selected effect on/off without removing it (Space).
        KeyCode::Char(' ') if state.focused_panel == FocusedPanel::EffectsList => {
            if !state.pipeline.effects.is_empty() {
                let ee = &mut state.pipeline.effects[state.selected_effect];
                ee.enabled = !ee.enabled;
                let enabled = ee.enabled;
                state.pipeline_dirty = true;
                state.status_message = if enabled {
                    "Effect enabled. Re-processing…".to_string()
                } else {
                    "Effect disabled. Re-processing…".to_string()
                };
                state.image_protocol = None;
                state.dispatch_process();
            }
        }
        // Delete selected effect.
        KeyCode::Delete | KeyCode::Char('d')
            if state.focused_panel == FocusedPanel::EffectsList =>
        {
            if !state.pipeline.effects.is_empty() {
                state.push_undo();
                state.pipeline.effects.remove(state.selected_effect);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.status_message = "Effect removed. Re-processing…".to_string();
                state.image_protocol = None;
                state.dispatch_process();
            }
        }
        // Randomize effect parameters.
        KeyCode::Char('r') => {
            state.push_undo();
            randomize_pipeline(&mut state.pipeline);
            state.pipeline_dirty = true;
            state.status_message = "Randomised pipeline. Re-processing…".to_string();
            state.image_protocol = None;
            state.dispatch_process();
        }
        // Open the edit-parameters modal (effects panel focused, effect selected).
        KeyCode::Enter
            if state.focused_panel == FocusedPanel::EffectsList
                && !state.pipeline.effects.is_empty() =>
        {
            let descriptors = state.pipeline.effects[state.selected_effect]
                .effect
                .param_descriptors();
            if descriptors.is_empty() {
                state.status_message = "This effect has no editable parameters.".to_string();
            } else {
                state.edit_params = descriptors
                    .iter()
                    .map(|d| format_param_value(d.value))
                    .collect();
                state.input_mode = InputMode::EditEffect { field_idx: 0 };
                state.status_message =
                    "Edit parameters (↑↓: field, Enter: apply, Esc: cancel)".to_string();
            }
        }
        // Open export dialog.
        KeyCode::Char('e') => {
            if state.preview_buffer.is_some() {
                // Populate dialog defaults from the current image path.
                let default_filename = state
                    .image_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| format!("{}_out", s.to_string_lossy()))
                    .unwrap_or_else(|| "output".to_string());
                state.export_dialog.filename = default_filename;
                state.export_dialog.format_index = 0;
                state.export_dialog.focused_field = 1;
                state.input_mode = InputMode::ExportDialog;
            } else {
                state.status_message = "No processed image to export.".to_string();
            }
        }
        // Decrease preview resolution.
        KeyCode::Char('[') => {
            if state.source_asset.is_some() && state.proxy_resolution_index > 0 {
                state.proxy_resolution_index -= 1;
                state.reload_proxy();
            }
        }
        // Increase preview resolution.
        KeyCode::Char(']') => {
            if state.source_asset.is_some()
                && state.proxy_resolution_index < PROXY_RESOLUTIONS.len() - 1
            {
                state.proxy_resolution_index += 1;
                state.reload_proxy();
            }
        }
        // Save the current pipeline via the save-pipeline dialog (Ctrl+S).
        KeyCode::Char('s') if modifiers.contains(KeyModifiers::CONTROL) => {
            if state.pipeline.effects.is_empty() {
                state.status_message = "Pipeline is empty – nothing to save.".to_string();
            } else {
                // Pre-populate with a sensible default filename.
                let default_filename = state
                    .image_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "pipeline".to_string());
                state.save_pipeline_dialog.filename = default_filename;
                state.save_pipeline_dialog.focused_field = 1;
                state.input_mode = InputMode::SavePipelineDialog;
            }
        }
        // Undo last pipeline edit (Ctrl+Z).
        KeyCode::Char('z') if modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(previous) = state.undo_stack.pop_front() {
                let current = std::mem::replace(&mut state.pipeline, previous);
                state.redo_stack.push_front(current);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.image_protocol = None;
                state.dispatch_process();
                state.status_message = format!(
                    "Undo – {} effect{} in pipeline.",
                    state.pipeline.effects.len(),
                    if state.pipeline.effects.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            } else {
                state.status_message = "Nothing to undo.".to_string();
            }
        }
        // Redo last undone pipeline edit (Ctrl+Y).
        KeyCode::Char('y') if modifiers.contains(KeyModifiers::CONTROL) => {
            if let Some(next) = state.redo_stack.pop_front() {
                let current = std::mem::replace(&mut state.pipeline, next);
                state.undo_stack.push_front(current);
                state.clamp_selection();
                state.pipeline_dirty = true;
                state.image_protocol = None;
                state.dispatch_process();
                state.status_message = format!(
                    "Redo – {} effect{} in pipeline.",
                    state.pipeline.effects.len(),
                    if state.pipeline.effects.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
            } else {
                state.status_message = "Nothing to redo.".to_string();
            }
        }
        // Load a pipeline from a JSON or YAML file via the file browser (Ctrl+L).
        KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            state.file_browser = Some(FileBrowserState::new(cwd, FileBrowserPurpose::LoadPipeline));
            state.input_mode = InputMode::FileBrowser;
        }
        // Clear all effects with a confirmation prompt (Ctrl+D).
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            if !state.pipeline.effects.is_empty() {
                state.input_mode = InputMode::ConfirmClearPipeline;
                state.status_message =
                    "Clear all effects? Press Enter to confirm or Esc to cancel.".to_string();
            } else {
                state.status_message = "Pipeline is already empty.".to_string();
            }
        }
        // Open the full keyboard-shortcut help overlay.
        KeyCode::Char('h') => {
            state.input_mode = InputMode::HelpModal;
        }
        // Toggle live histogram overlay.
        KeyCode::Char('H') => {
            state.show_histogram = !state.show_histogram;
            state.status_message = if state.show_histogram {
                "Histogram overlay enabled.".to_string()
            } else {
                "Histogram overlay disabled.".to_string()
            };
        }
        // Toggle side-by-side before/after split view.
        KeyCode::Char('v') => {
            state.split_view = !state.split_view;
            state.status_message = if state.split_view {
                "Split view enabled – left: original, right: processed.".to_string()
            } else {
                "Split view disabled.".to_string()
            };
        }
        _ => {}
    }
}

/// Move the selected effect one position up in the pipeline.
fn move_effect_up(state: &mut AppState) {
    let idx = state.selected_effect;
    if idx > 0 {
        state.push_undo();
        state.pipeline.effects.swap(idx, idx - 1);
        state.selected_effect -= 1;
        state.dragging_effect = true;
        state.pipeline_dirty = true;
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
        state.push_undo();
        state.pipeline.effects.swap(idx, idx + 1);
        state.selected_effect += 1;
        state.dragging_effect = true;
        state.pipeline_dirty = true;
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
            if state.add_effect_cursor > 0 {
                state.add_effect_cursor -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.add_effect_cursor < n - 1 {
                state.add_effect_cursor += 1;
            }
        }
        KeyCode::Enter => {
            let effect = AVAILABLE_EFFECTS[state.add_effect_cursor].1();
            state.push_undo();
            state.pipeline.effects.push(EnabledEffect::new(effect));
            state.input_mode = InputMode::Normal;
            state.selected_effect = state.pipeline.effects.len() - 1;
            state.pipeline_dirty = true;
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
            let purpose = state.file_browser.as_ref().map(|fb| fb.purpose.clone());
            match action {
                Some(FileBrowserEntry::Directory(_)) => {
                    if let Some(ref mut fb) = state.file_browser {
                        fb.enter_dir();
                    }
                }
                Some(FileBrowserEntry::ImageFile(path, _)) => {
                    state.input_mode = InputMode::Normal;
                    state.file_browser = None;
                    match purpose {
                        Some(FileBrowserPurpose::LoadPipeline) => {
                            match crate::config::parser::load_pipeline(&path) {
                                Ok(pipeline) => {
                                    let effect_count = pipeline.effects.len();
                                    let filename = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| path.display().to_string());
                                    state.pipeline = pipeline;
                                    state.clamp_selection();
                                    state.pipeline_dirty = false;
                                    state.quit_requested = false;
                                    state.undo_stack.clear();
                                    state.redo_stack.clear();
                                    state.image_protocol = None;
                                    state.dispatch_process();
                                    state.status_message = format!(
                                        "Loaded {effect_count} effect{} from {filename}",
                                        if effect_count == 1 { "" } else { "s" }
                                    );
                                }
                                Err(e) => {
                                    state.status_message = format!("Error loading pipeline: {e}");
                                }
                            }
                        }
                        _ => {
                            state.load_image(path);
                        }
                    }
                }
                None => {}
            }
        }
        _ => {}
    }
}

fn handle_edit_effect(state: &mut AppState, code: KeyCode) {
    let field_idx = match state.input_mode {
        InputMode::EditEffect { field_idx } => field_idx,
        _ => return,
    };

    let num_fields = if state.pipeline.effects.is_empty() {
        0
    } else {
        state.pipeline.effects[state.selected_effect]
            .effect
            .param_descriptors()
            .len()
    };

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.edit_params.clear();
            state.status_message = "Edit cancelled.".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if field_idx > 0 {
                state.input_mode = InputMode::EditEffect {
                    field_idx: field_idx - 1,
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if field_idx + 1 < num_fields {
                state.input_mode = InputMode::EditEffect {
                    field_idx: field_idx + 1,
                };
            }
        }
        KeyCode::Backspace => {
            if let Some(buf) = state.edit_params.get_mut(field_idx) {
                buf.pop();
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() || c == '.' || c == '-' => {
            if let Some(buf) = state.edit_params.get_mut(field_idx) {
                buf.push(c);
            }
        }
        KeyCode::Enter => {
            if !state.pipeline.effects.is_empty() {
                let descriptors = state.pipeline.effects[state.selected_effect]
                    .effect
                    .param_descriptors();
                let values: Vec<f32> = state
                    .edit_params
                    .iter()
                    .zip(descriptors.iter())
                    .map(|(s, d)| s.parse::<f32>().unwrap_or(d.value).clamp(d.min, d.max))
                    .collect();
                let updated = state.pipeline.effects[state.selected_effect]
                    .effect
                    .apply_params(&values);
                state.push_undo();
                state.pipeline.effects[state.selected_effect].effect = updated;
                state.pipeline_dirty = true;
                state.status_message = "Effect updated. Re-processing…".to_string();
                state.image_protocol = None;
                state.dispatch_process();
            }
            state.input_mode = InputMode::Normal;
            state.edit_params.clear();
        }
        _ => {}
    }
}

/// Format a float parameter value for display in the edit buffer.
///
/// Integers (where `fract() == 0`) are displayed without a decimal point
/// (e.g. `8` instead of `8`), while fractional values use Rust's default
/// shortest-round-trip representation (e.g. `0.05`, `1.5`).
fn format_param_value(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn handle_export_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_DIRECTORY: usize = 0;
    const FIELD_FILENAME: usize = 1;
    const FIELD_FORMAT: usize = 2;
    const FIELD_COUNT: usize = 3;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Export cancelled.".to_string();
        }
        KeyCode::Enter => {
            // Build the output path from dialog state.
            let format = EXPORT_FORMATS[state.export_dialog.format_index].clone();
            let ext = format.extension();
            let dir = std::path::PathBuf::from(&state.export_dialog.directory);
            let filename = state.export_dialog.effective_filename().to_string();
            let output_path = dir.join(format!("{filename}.{ext}"));
            state.dispatch_export(output_path, format);
            state.input_mode = InputMode::Normal;
            state.status_message = "Exporting…".to_string();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.export_dialog.focused_field > 0 {
                state.export_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.export_dialog.focused_field < FIELD_COUNT - 1 {
                state.export_dialog.focused_field += 1;
            }
        }
        // Format cycling.
        KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
            if state.export_dialog.focused_field == FIELD_FORMAT =>
        {
            let n = EXPORT_FORMATS.len();
            state.export_dialog.format_index = match code {
                KeyCode::Left => {
                    if state.export_dialog.format_index == 0 {
                        n - 1
                    } else {
                        state.export_dialog.format_index - 1
                    }
                }
                _ => (state.export_dialog.format_index + 1) % n,
            };
        }
        // Text editing for Directory and Filename fields.
        KeyCode::Backspace => match state.export_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.export_dialog.directory.pop();
            }
            FIELD_FILENAME => {
                state.export_dialog.filename.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.export_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.export_dialog.directory.push(c);
            }
            FIELD_FILENAME => {
                state.export_dialog.filename.push(c);
            }
            _ => {}
        },
        _ => {}
    }
}

/// Handle keyboard input when the user is configuring a pipeline save via the dialog.
fn handle_save_pipeline_dialog(state: &mut AppState, code: KeyCode) {
    const FIELD_DIRECTORY: usize = 0;
    const FIELD_FILENAME: usize = 1;
    const FIELD_COUNT: usize = 2;

    match code {
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Save cancelled.".to_string();
        }
        KeyCode::Enter => {
            let dir = std::path::PathBuf::from(&state.save_pipeline_dialog.directory);
            let filename = state.save_pipeline_dialog.effective_filename().to_string();
            // Enforce the .json extension regardless of what the user typed.
            let output_path = dir.join(format!("{filename}.json"));
            match crate::config::parser::save_pipeline(&state.pipeline, &output_path) {
                Ok(()) => {
                    state.pipeline_dirty = false;
                    state.quit_requested = false;
                    state.status_message = format!("Pipeline saved → {}", output_path.display());
                }
                Err(e) => {
                    state.status_message = format!("Error saving pipeline: {e}");
                }
            }
            state.input_mode = InputMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.save_pipeline_dialog.focused_field > 0 {
                state.save_pipeline_dialog.focused_field -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.save_pipeline_dialog.focused_field < FIELD_COUNT - 1 {
                state.save_pipeline_dialog.focused_field += 1;
            }
        }
        KeyCode::Backspace => match state.save_pipeline_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.save_pipeline_dialog.directory.pop();
            }
            FIELD_FILENAME => {
                state.save_pipeline_dialog.filename.pop();
            }
            _ => {}
        },
        KeyCode::Char(c) => match state.save_pipeline_dialog.focused_field {
            FIELD_DIRECTORY => {
                state.save_pipeline_dialog.directory.push(c);
            }
            FIELD_FILENAME => {
                state.save_pipeline_dialog.filename.push(c);
            }
            _ => {}
        },
        _ => {}
    }
}

/// Handle keyboard input when the full help overlay is shown.
fn handle_help_modal(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => {
            state.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}

/// Handle the confirmation prompt for clearing all pipeline effects (Ctrl+D).
fn handle_confirm_clear_pipeline(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            state.push_undo();
            state.pipeline.effects.clear();
            state.selected_effect = 0;
            state.pipeline_dirty = true;
            state.image_protocol = None;
            state.dispatch_process();
            state.input_mode = InputMode::Normal;
            state.status_message = "Pipeline cleared.".to_string();
        }
        KeyCode::Esc => {
            state.input_mode = InputMode::Normal;
            state.status_message = "Clear cancelled.".to_string();
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
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((rng >> 33) as f32) / (u32::MAX as f32)
    };

    // Populate the pipeline with 2-5 random effects so randomization is
    // visible even when starting from an empty pipeline.
    let count = 2 + (next() * 4.0) as usize;
    pipeline.effects.clear();
    for _ in 0..count {
        let idx = (next() * AVAILABLE_EFFECTS.len() as f32) as usize % AVAILABLE_EFFECTS.len();
        pipeline.effects.push(EnabledEffect::new(AVAILABLE_EFFECTS[idx].1()));
    }

    for ee in &mut pipeline.effects {
        match &mut ee.effect {
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

    #[test]
    fn randomize_pipeline_populates_empty_pipeline() {
        let mut pipeline = Pipeline::default();
        assert!(pipeline.effects.is_empty(), "pipeline should start empty");

        randomize_pipeline(&mut pipeline);

        let len = pipeline.effects.len();
        assert!(
            (2..=5).contains(&len),
            "randomize should add 2–5 effects, got {len}"
        );
    }

    #[test]
    fn randomize_pipeline_replaces_existing_effects() {
        let mut pipeline = Pipeline::default();
        randomize_pipeline(&mut pipeline);
        let first_len = pipeline.effects.len();

        // A second call must also produce a valid count (pipeline is non-empty now).
        randomize_pipeline(&mut pipeline);
        let second_len = pipeline.effects.len();

        assert!(
            (2..=5).contains(&first_len),
            "first randomize should give 2–5 effects, got {first_len}"
        );
        assert!(
            (2..=5).contains(&second_len),
            "second randomize should give 2–5 effects, got {second_len}"
        );
    }

    fn make_state_with_effects() -> AppState {
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        let mut state = AppState::new(worker_tx, resp_rx, resp_tx, picker);
        state.pipeline = Pipeline {
            effects: vec![
                EnabledEffect::new(Effect::Color(ColorEffect::Invert)),
                EnabledEffect::new(Effect::Glitch(GlitchEffect::Pixelate { block_size: 8 })),
                EnabledEffect::new(Effect::Color(ColorEffect::HueShift { degrees: 30.0 })),
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
        assert!(matches!(state.pipeline.effects[0].effect, Effect::Glitch(_)));
        assert!(matches!(
            state.pipeline.effects[1].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_up_with_shift_up() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Up, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 0);
        assert!(matches!(state.pipeline.effects[0].effect, Effect::Glitch(_)));
        assert!(matches!(
            state.pipeline.effects[1].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(state.status_message.contains("up"));
    }

    #[test]
    fn move_effect_down_with_j() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('J'), KeyModifiers::NONE);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1].effect, Effect::Color(_)));
        assert!(matches!(state.pipeline.effects[2].effect, Effect::Glitch(_)));
        assert!(state.status_message.contains("down"));
    }

    #[test]
    fn move_effect_down_with_shift_down() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Down, KeyModifiers::SHIFT);
        assert_eq!(state.selected_effect, 2);
        assert!(matches!(state.pipeline.effects[1].effect, Effect::Color(_)));
        assert!(matches!(state.pipeline.effects[2].effect, Effect::Glitch(_)));
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
        assert!(matches!(
            state.pipeline.effects[0].effect,
            Effect::Color(ColorEffect::Invert)
        ));
    }

    // ── Pipeline save / load ──────────────────────────────────────────────────

    fn make_state_empty() -> AppState {
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        AppState::new(worker_tx, resp_rx, resp_tx, picker)
    }

    #[test]
    fn ctrl_s_on_empty_pipeline_shows_error() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::Normal, "mode must stay Normal");
        assert!(
            state.status_message.contains("empty"),
            "status should mention empty pipeline: {}",
            state.status_message
        );
    }

    #[test]
    fn ctrl_s_with_effects_enters_save_dialog() {
        let mut state = make_state_with_effects();
        handle_normal(&mut state, KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::SavePipelineDialog);
        // Dialog should be pre-populated with a non-empty default filename.
        assert!(
            !state.save_pipeline_dialog.filename.is_empty(),
            "default filename should be populated"
        );
    }

    #[test]
    fn save_pipeline_dialog_esc_cancels() {
        let mut state = make_state_with_effects();
        state.input_mode = InputMode::SavePipelineDialog;
        state.save_pipeline_dialog.filename = "something".to_string();
        handle_save_pipeline_dialog(&mut state, KeyCode::Esc);
        assert_eq!(state.input_mode, InputMode::Normal);
        assert!(
            state.status_message.contains("cancel") || state.status_message.contains("Cancel"),
            "status should mention cancellation: {}",
            state.status_message
        );
    }

    #[test]
    fn save_pipeline_dialog_enter_returns_to_normal() {
        let mut state = make_state_with_effects();
        state.input_mode = InputMode::SavePipelineDialog;
        // Point the dialog at the temp dir so we don't pollute the project root.
        state.save_pipeline_dialog.directory = std::env::temp_dir().to_string_lossy().into_owned();
        // Leave filename empty — effective_filename() will fall back to "pipeline".
        handle_save_pipeline_dialog(&mut state, KeyCode::Enter);
        assert_eq!(state.input_mode, InputMode::Normal);
        // Clean up any file that was created.
        let _ = std::fs::remove_file(std::env::temp_dir().join("pipeline.json"));
    }

    #[test]
    fn save_pipeline_dialog_enforces_json_extension() {
        let tmp_dir = std::env::temp_dir();
        let (worker_tx, _worker_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = std::sync::mpsc::channel();
        let picker = ratatui_image::picker::Picker::halfblocks();
        let mut state = AppState::new(worker_tx, resp_rx, resp_tx, picker);
        state.pipeline = Pipeline {
            effects: vec![EnabledEffect::new(Effect::Color(ColorEffect::Invert))],
        };
        state.input_mode = InputMode::SavePipelineDialog;
        state.save_pipeline_dialog.directory = tmp_dir.to_string_lossy().into_owned();
        state.save_pipeline_dialog.filename = "test_enforce_ext".to_string();
        handle_save_pipeline_dialog(&mut state, KeyCode::Enter);
        // Resulting path must carry the .json extension.
        let expected = tmp_dir.join("test_enforce_ext.json");
        assert!(
            expected.exists(),
            "saved file should have .json extension at {}",
            expected.display()
        );
        let _ = std::fs::remove_file(&expected);
    }

    #[test]
    fn help_modal_opens_and_closes() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(
            state.input_mode,
            InputMode::HelpModal,
            "h should open HelpModal"
        );
        handle_help_modal(&mut state, KeyCode::Esc);
        assert_eq!(
            state.input_mode,
            InputMode::Normal,
            "Esc should close HelpModal"
        );
    }

    #[test]
    fn help_modal_closed_by_h() {
        let mut state = make_state_empty();
        state.input_mode = InputMode::HelpModal;
        handle_help_modal(&mut state, KeyCode::Char('h'));
        assert_eq!(state.input_mode, InputMode::Normal);
    }

    #[test]
    fn save_pipeline_roundtrip() {
        let tmp = std::env::temp_dir().join("spixelatuir_test_pipeline.json");
        let pipeline = Pipeline {
            effects: vec![
                EnabledEffect::new(Effect::Color(ColorEffect::Invert)),
                EnabledEffect::new(Effect::Glitch(GlitchEffect::Pixelate { block_size: 4 })),
            ],
        };

        crate::config::parser::save_pipeline(&pipeline, &tmp).expect("save should succeed");

        let loaded = crate::config::parser::load_pipeline(&tmp).expect("load should succeed");

        assert_eq!(loaded.effects.len(), 2);
        assert!(matches!(
            loaded.effects[0].effect,
            Effect::Color(ColorEffect::Invert)
        ));
        assert!(matches!(
            loaded.effects[1].effect,
            Effect::Glitch(GlitchEffect::Pixelate { block_size: 4 })
        ));
        assert!(loaded.effects[0].enabled, "loaded effects should be enabled by default");
        assert!(loaded.effects[1].enabled, "all loaded effects should be enabled by default");

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn ctrl_l_enters_file_browser_with_pipeline_purpose() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('l'), KeyModifiers::CONTROL);
        assert_eq!(state.input_mode, InputMode::FileBrowser);
        assert!(
            state
                .file_browser
                .as_ref()
                .map(|fb| fb.purpose == FileBrowserPurpose::LoadPipeline)
                .unwrap_or(false),
            "file browser should have LoadPipeline purpose"
        );
    }

    #[test]
    fn ctrl_o_enters_file_browser_with_open_image_purpose() {
        let mut state = make_state_empty();
        handle_normal(&mut state, KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(state.input_mode, InputMode::FileBrowser);
        assert!(
            state
                .file_browser
                .as_ref()
                .map(|fb| fb.purpose == FileBrowserPurpose::OpenImage)
                .unwrap_or(false),
            "file browser should have OpenImage purpose"
        );
    }
}
