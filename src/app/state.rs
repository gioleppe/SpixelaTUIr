use std::collections::VecDeque;
use std::sync::mpsc;

use ratatui::layout::Rect;
use ratatui_image::{Resize, ResizeEncodeRender, picker::Picker, protocol::StatefulProtocol};

use crate::effects::Pipeline;
use crate::engine::export::ExportFormat;
use crate::engine::worker::{WorkerCommand, WorkerResponse};

use super::PROXY_RESOLUTIONS;
use super::animation::{
    AnimationExportDialogState, AnimationFrame, AnimationPlaybackState, AnimationTimeline,
    SweepDialogState,
};
use super::dialogs::{ExportDialogState, FocusedPanel, InputMode, SavePipelineDialogState};
use super::file_browser::FileBrowserState;

/// Central application state.
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
    /// Ring-buffer of past pipeline states for undo (most-recent-first).
    pub undo_stack: VecDeque<Pipeline>,
    /// Stack of pipeline states that were undone, for redo.
    pub redo_stack: VecDeque<Pipeline>,
    /// Whether to show the live luminance/RGB histogram overlay on the canvas.
    pub show_histogram: bool,
    /// Whether the canvas is in side-by-side before/after split view.
    pub split_view: bool,
    /// ratatui-image stateful protocol for displaying the original (pre-effects) proxy.
    /// Only populated when `split_view` is active and an image is loaded.
    pub original_image_protocol: Option<StatefulProtocol>,
    /// ratatui-image stateful protocol for the file-browser thumbnail preview.
    /// Populated while the file-browser modal is open and an image entry is highlighted.
    pub file_browser_preview: Option<StatefulProtocol>,
    /// The screen area that `image_protocol` was last rendered into.
    ///
    /// Stored so that `set_preview` can immediately pre-encode the replacement
    /// protocol with the same area, preventing the Sixel clear+redraw blink that
    /// would otherwise occur on the first render after the protocol is replaced.
    pub image_protocol_last_area: Option<Rect>,

    // ── Animation ───────────────────────────────────────────────────────────
    /// The full animation timeline (frames + global settings).
    pub animation: AnimationTimeline,
    /// Whether the animation panel strip is visible at the bottom.
    pub animation_panel_open: bool,
    /// Current playback state (stopped / playing / paused).
    pub animation_playback: AnimationPlaybackState,
    /// Pre-rendered proxy images for each animation frame.
    ///
    /// Indexed by frame position; `None` means not yet rendered or dirty.
    /// Populated by the worker in response to `RenderAnimationFrame` /
    /// `RenderSweepBatch` commands.
    pub animation_rendered_frames: Vec<Option<image::DynamicImage>>,
    /// How many frames are still waiting for the worker to render (sweep batch).
    pub animation_pending_renders: usize,
    /// State for the animation export dialog.
    pub animation_export_dialog: AnimationExportDialogState,
    /// State for the parameter sweep dialog.
    pub sweep_dialog: SweepDialogState,
    /// Inline text buffer for per-frame duration editing (`f` key).
    pub frame_duration_input: String,
    /// When `true`, the pending duration input (`f` edit) applies to all frames.
    /// When `false`, it applies only to `animation.selected`.
    pub frame_duration_input_all: bool,
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
            export_dialog: ExportDialogState::default(),
            save_pipeline_dialog: SavePipelineDialogState::default(),
            // Default to index 1 (512 px) — preserves prior behaviour.
            proxy_resolution_index: 1,
            dragging_effect: false,
            pipeline_dirty: false,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            show_histogram: false,
            split_view: false,
            original_image_protocol: None,
            file_browser_preview: None,
            image_protocol_last_area: None,
            animation: AnimationTimeline::default(),
            animation_panel_open: false,
            animation_playback: AnimationPlaybackState::Stopped,
            animation_rendered_frames: Vec::new(),
            animation_pending_renders: 0,
            animation_export_dialog: AnimationExportDialogState::default(),
            sweep_dialog: SweepDialogState::default(),
            frame_duration_input: String::new(),
            frame_duration_input_all: false,
        }
    }

    /// Load an image from disk, create a proxy, and dispatch to the worker thread.
    pub fn load_image(&mut self, path: std::path::PathBuf) {
        log::info!("Loading image: {}", path.display());
        match image::open(&path) {
            Ok(img) => {
                let size = PROXY_RESOLUTIONS[self.proxy_resolution_index];
                log::debug!(
                    "Image loaded ({}x{}), creating proxy at {size}px",
                    img.width(),
                    img.height()
                );
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
                log::error!("Failed to open image {}: {e}", path.display());
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
            log::debug!(
                "Dispatching process: {} enabled effect(s), proxy {}x{}",
                self.pipeline.effects.iter().filter(|e| e.enabled).count(),
                proxy.width(),
                proxy.height()
            );
            let _ = self.worker_tx.send(WorkerCommand::Process {
                image: proxy.clone(),
                pipeline: self.pipeline.clone(),
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }

    /// Request a thumbnail for `path` from the worker thread, to populate
    /// the file-browser preview pane.  Any previous preview is cleared
    /// immediately so stale images are never shown while the new one loads.
    pub fn dispatch_file_browser_preview(&mut self, path: std::path::PathBuf) {
        self.file_browser_preview = None;
        let _ = self.worker_tx.send(
            crate::engine::worker::WorkerCommand::LoadFileBrowserPreview {
                path,
                response_tx: self.worker_resp_tx.clone(),
            },
        );
    }

    /// Re-scale the proxy from `source_asset` at the current resolution tier and
    /// re-dispatch the pipeline.  Called from keyboard shortcuts `[` / `]`.
    pub fn reload_proxy(&mut self) {
        if let Some(source) = self.source_asset.take() {
            let size = PROXY_RESOLUTIONS[self.proxy_resolution_index];
            log::debug!("Reloading proxy at {size}px");
            let proxy = source.thumbnail(size, size);
            self.source_asset = Some(source);
            self.original_image_protocol = Some(self.picker.new_resize_protocol(proxy.clone()));
            self.proxy_asset = Some(proxy);
            self.preview_buffer = None;
            self.image_protocol = None;
            self.dispatch_process();
            self.status_message = format!("Preview resolution: {size}px — Re-processing…");
        }
    }

    /// Update the displayed image from a processed frame.
    ///
    /// Creates a new `StatefulProtocol` for the incoming image. If the render
    /// area from the previous frame is known, the protocol is immediately
    /// pre-encoded for that area so that `needs_resize` returns `None` on the
    /// very next render pass — avoiding the Sixel clear+redraw blink that would
    /// otherwise occur because a fresh protocol starts with `hash = 0`.
    pub fn set_preview(&mut self, img: image::DynamicImage) {
        log::debug!("Preview updated ({}x{})", img.width(), img.height());
        let mut new_protocol = self.picker.new_resize_protocol(img.clone());
        // Pre-encode the protocol for the last known render area so the
        // next `render_stateful_widget` call sees no change and skips
        // the expensive re-encode (and associated terminal blink).
        let resize = Resize::Fit(None);
        if let Some(screen_area) = self.image_protocol_last_area
            && let Some(target) = new_protocol.needs_resize(&resize, screen_area)
        {
            new_protocol.resize_encode(&resize, target);
        }
        self.image_protocol = Some(new_protocol);
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
            log::info!(
                "Dispatching export: {} as {}",
                output_path.display(),
                format.display_name()
            );
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

    /// Mutate the pipeline with automatic undo, dirty-tracking, and re-processing.
    ///
    /// Performs the following sequence automatically:
    /// 1. Pushes the current pipeline onto the **undo stack** (clears redo).
    /// 2. Applies the caller-supplied closure `f` to the pipeline.
    /// 3. Sets `pipeline_dirty = true`.
    /// 4. Clears the cached `image_protocol` (forces a Sixel re-encode).
    /// 5. Dispatches the updated pipeline to the worker thread for processing.
    ///
    /// Use this helper for simple pipeline mutations where no additional
    /// state adjustments (e.g. `clamp_selection`) are needed after the change.
    pub fn mutate_pipeline(&mut self, f: impl FnOnce(&mut Pipeline)) {
        self.push_undo();
        f(&mut self.pipeline);
        self.pipeline_dirty = true;
        self.image_protocol = None;
        self.dispatch_process();
    }

    // ── Animation helpers ────────────────────────────────────────────────────

    /// Capture the current pipeline as a new animation frame.
    ///
    /// Inserts after the currently selected frame (or appends if empty).
    /// Returns the index of the newly inserted frame.
    pub fn capture_animation_frame(&mut self) -> usize {
        let frame = AnimationFrame {
            pipeline: self.pipeline.clone(),
            duration_ms: 0, // use global fps
            label: None,
        };
        let insert_at = if self.animation.frames.is_empty() {
            0
        } else {
            self.animation.selected + 1
        };
        let insert_at = insert_at.min(self.animation.frames.len());
        self.animation.frames.insert(insert_at, frame);
        // Grow the rendered-frames cache to match.
        self.animation_rendered_frames.insert(insert_at, None);
        self.animation.selected = insert_at;

        // Dispatch render for the new frame.
        self.dispatch_render_animation_frame(insert_at);
        insert_at
    }

    /// Dispatch a single-frame render for animation frame `idx`.
    pub fn dispatch_render_animation_frame(&self, idx: usize) {
        if let Some(proxy) = &self.proxy_asset {
            if let Some(frame) = self.animation.frames.get(idx) {
                let _ = self.worker_tx.send(WorkerCommand::RenderAnimationFrame {
                    image: proxy.clone(),
                    pipeline: frame.pipeline.clone(),
                    frame_idx: idx,
                    response_tx: self.worker_resp_tx.clone(),
                });
            }
        }
    }

    /// Dispatch rendering for all animation frames that are not yet rendered
    /// (i.e. `animation_rendered_frames[i]` is `None`).
    pub fn dispatch_render_dirty_frames(&mut self) {
        let count = self
            .animation_rendered_frames
            .iter()
            .filter(|f| f.is_none())
            .count();
        if count > 0 {
            self.animation_pending_renders += count;
            let total = self.animation.frames.len();
            for i in 0..total {
                if matches!(self.animation_rendered_frames.get(i), None | Some(None)) {
                    self.dispatch_render_animation_frame(i);
                }
            }
        }
    }

    /// Dispatch a sweep batch: interpolated pipelines for each frame.
    pub fn dispatch_sweep_batch(&self, pipelines: Vec<Pipeline>) {
        if let Some(proxy) = &self.proxy_asset {
            let _ = self.worker_tx.send(WorkerCommand::RenderSweepBatch {
                image: proxy.clone(),
                pipelines,
                response_tx: self.worker_resp_tx.clone(),
            });
        }
    }

    /// Receive a rendered animation frame from the worker and store it.
    pub fn receive_animation_frame(&mut self, frame_idx: usize, img: image::DynamicImage) {
        if frame_idx < self.animation_rendered_frames.len() {
            self.animation_rendered_frames[frame_idx] = Some(img);
        }
        if self.animation_pending_renders > 0 {
            self.animation_pending_renders -= 1;
        }
    }

    /// Replace all animation frames with sweep results.
    ///
    /// `pipelines` contains one pipeline per frame; `images` contains the
    /// pre-rendered preview for each pipeline (same length).
    pub fn apply_sweep_results(
        &mut self,
        pipelines: Vec<Pipeline>,
        images: Vec<image::DynamicImage>,
    ) {
        self.animation.frames = pipelines
            .into_iter()
            .map(|pipeline| AnimationFrame {
                pipeline,
                duration_ms: 0,
                label: None,
            })
            .collect();
        self.animation_rendered_frames = images.into_iter().map(Some).collect();
        self.animation.selected = 0;
        self.animation_pending_renders = 0;
        // Show the first frame in the canvas.
        if let Some(Some(img)) = self.animation_rendered_frames.first() {
            let img = img.clone();
            self.set_preview(img);
        }
    }

    /// Clamp `animation.selected` to valid bounds.
    pub fn clamp_animation_selection(&mut self) {
        let len = self.animation.frames.len();
        if len == 0 {
            self.animation.selected = 0;
        } else {
            self.animation.selected = self.animation.selected.min(len - 1);
        }
    }

    /// Load the pipeline from the selected animation frame back into the live
    /// effects pipeline so the user can edit it.
    pub fn load_animation_frame_pipeline(&mut self) {
        let idx = self.animation.selected;
        if let Some(pipeline) = self.animation.frames.get(idx).map(|f| f.pipeline.clone()) {
            self.push_undo();
            self.pipeline = pipeline;
            self.pipeline_dirty = true;
            self.clamp_selection();
            self.image_protocol = None;
            self.dispatch_process();
        }
    }
}
