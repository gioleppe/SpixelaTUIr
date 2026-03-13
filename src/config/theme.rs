use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub active_border: Option<String>,
    pub inactive_border: Option<String>,
    pub text_normal: Option<String>,
    pub text_dimmed: Option<String>,
    pub selection_bg: Option<String>,
    pub selection_fg: Option<String>,
    pub selection_inactive_bg: Option<String>,
    pub directory: Option<String>,
    pub error_border: Option<String>,
    pub success_border: Option<String>,
    pub warning_border: Option<String>,
    pub accent_1: Option<String>, // e.g. Magenta
    pub accent_2: Option<String>, // e.g. Cyan
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub active_border: Color,
    pub inactive_border: Color,
    pub text_normal: Color,
    pub text_dimmed: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub selection_inactive_bg: Color,
    pub directory: Color,
    pub error_border: Color,
    pub success_border: Color,
    pub warning_border: Color,
    pub accent_1: Color,
    pub accent_2: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            active_border: Color::Cyan,
            inactive_border: Color::DarkGray,
            text_normal: Color::White,
            text_dimmed: Color::DarkGray,
            selection_bg: Color::Cyan,
            selection_fg: Color::Black,
            selection_inactive_bg: Color::Yellow,
            directory: Color::Blue,
            error_border: Color::Red,
            success_border: Color::Green,
            warning_border: Color::Yellow,
            accent_1: Color::Magenta,
            accent_2: Color::Cyan,
        }
    }
}

impl Theme {
    pub fn load_from_path(path: &Path) -> Self {
        let mut theme = Self::default();
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str::<ThemeConfig>(&contents) {
                if let Some(c) = config.active_border.and_then(|s| Color::from_str(&s).ok()) {
                    theme.active_border = c;
                }
                if let Some(c) = config
                    .inactive_border
                    .and_then(|s| Color::from_str(&s).ok())
                {
                    theme.inactive_border = c;
                }
                if let Some(c) = config.text_normal.and_then(|s| Color::from_str(&s).ok()) {
                    theme.text_normal = c;
                }
                if let Some(c) = config.text_dimmed.and_then(|s| Color::from_str(&s).ok()) {
                    theme.text_dimmed = c;
                }
                if let Some(c) = config.selection_bg.and_then(|s| Color::from_str(&s).ok()) {
                    theme.selection_bg = c;
                }
                if let Some(c) = config.selection_fg.and_then(|s| Color::from_str(&s).ok()) {
                    theme.selection_fg = c;
                }
                if let Some(c) = config
                    .selection_inactive_bg
                    .and_then(|s| Color::from_str(&s).ok())
                {
                    theme.selection_inactive_bg = c;
                }
                if let Some(c) = config.directory.and_then(|s| Color::from_str(&s).ok()) {
                    theme.directory = c;
                }
                if let Some(c) = config.error_border.and_then(|s| Color::from_str(&s).ok()) {
                    theme.error_border = c;
                }
                if let Some(c) = config.success_border.and_then(|s| Color::from_str(&s).ok()) {
                    theme.success_border = c;
                }
                if let Some(c) = config.warning_border.and_then(|s| Color::from_str(&s).ok()) {
                    theme.warning_border = c;
                }
                if let Some(c) = config.accent_1.and_then(|s| Color::from_str(&s).ok()) {
                    theme.accent_1 = c;
                }
                if let Some(c) = config.accent_2.and_then(|s| Color::from_str(&s).ok()) {
                    theme.accent_2 = c;
                }
            }
        }
        theme
    }

    /// Tries to load the theme from the default user configuration directory
    /// (e.g. `~/.config/spix/theme.json`). Returns `Self::default()` if not found.
    pub fn load_default() -> Self {
        if let Some(mut path) = dirs::config_dir() {
            path.push("spix");
            path.push("theme.json");
            Self::load_from_path(&path)
        } else {
            Self::default()
        }
    }
}
