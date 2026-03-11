use std::path::PathBuf;

/// A single entry in the file browser – either a directory or a selectable file.
#[derive(Debug, Clone)]
pub enum FileBrowserEntry {
    Directory(PathBuf),
    /// Path and pre-fetched file size in bytes.
    ImageFile(PathBuf, u64),
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
    pub cwd: PathBuf,
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
    pub fn new(dir: PathBuf, purpose: FileBrowserPurpose) -> Self {
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
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

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
