use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted list of favorited effect names (matching `EffectEntry.0`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FavoritesConfig {
    /// Names of effects the user has marked as favorite.
    pub favorites: Vec<String>,
}

impl FavoritesConfig {
    /// Return the default on-disk path: `~/.config/spix/favorites.json`.
    pub fn config_path() -> Option<PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("spix");
        path.push("favorites.json");
        Some(path)
    }

    /// Load favorites from disk, falling back to an empty list on any error.
    pub fn load() -> Self {
        if let Some(path) = Self::config_path()
            && let Ok(data) = crate::config::read_to_string_limited(&path, 1_000_000)
            && let Ok(cfg) = serde_json::from_str::<Self>(&data)
        {
            return cfg;
        }
        Self::default()
    }

    /// Persist favorites to disk, creating parent directories as needed.
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(data) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&path, data);
            }
        }
    }

    /// Returns `true` if `name` is currently favorited.
    pub fn is_favorite(&self, name: &str) -> bool {
        self.favorites.iter().any(|f| f == name)
    }

    /// Toggle the favorite status of `name`, then save to disk.
    pub fn toggle(&mut self, name: &str) {
        if let Some(pos) = self.favorites.iter().position(|f| f == name) {
            self.favorites.remove(pos);
        } else {
            self.favorites.push(name.to_owned());
        }
        self.save();
    }
}
