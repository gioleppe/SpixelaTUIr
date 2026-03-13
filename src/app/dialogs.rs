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

impl Default for ExportDialogState {
    fn default() -> Self {
        Self {
            directory: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .to_string_lossy()
                .into_owned(),
            filename: String::new(),
            format_index: 0,
            focused_field: 1,
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

impl Default for SavePipelineDialogState {
    fn default() -> Self {
        Self {
            directory: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .to_string_lossy()
                .into_owned(),
            filename: String::new(),
            focused_field: 1,
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
    /// Waiting for the user to confirm quitting with unsaved changes.
    ConfirmQuit,
    /// Animation panel has keyboard focus (frame capture, navigation, playback).
    AnimationPanel,
    /// User is filling in the parameter-sweep dialog.
    AnimationSweepDialog,
    /// User is configuring animation export (GIF/WebP) via the export dialog.
    AnimationExportDialog,
    /// User is typing a new duration value for the selected frame (inline `f` edit).
    AnimationFrameDurationInput,
}

impl InputMode {
    /// Returns true if the current input mode represents a floating modal or overlay.
    pub fn is_modal(&self) -> bool {
        matches!(
            self,
            InputMode::PathInput
                | InputMode::AddEffect
                | InputMode::FileBrowser
                | InputMode::EditEffect { .. }
                | InputMode::ExportDialog
                | InputMode::SavePipelineDialog
                | InputMode::HelpModal
                | InputMode::ConfirmClearPipeline
                | InputMode::ConfirmQuit
                | InputMode::AnimationSweepDialog
                | InputMode::AnimationExportDialog
                | InputMode::AnimationFrameDurationInput
        )
    }
}

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Canvas,
    EffectsList,
    /// The animation panel at the bottom of the screen.
    AnimationPanel,
}
