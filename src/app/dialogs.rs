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
}

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, PartialEq)]
pub enum FocusedPanel {
    Canvas,
    EffectsList,
}
