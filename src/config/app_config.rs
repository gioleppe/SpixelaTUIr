//! Application-wide user configuration loaded from `~/.config/spix/config.json`.
//!
//! Currently exposes:
//!
//! * `proxy_resolutions` — overrides the built-in list of preview-resolution
//!   tiers cycled with `[` / `]`. Useful for users on very fast machines who
//!   want higher-fidelity live previews, or on slow machines who want to cap
//!   the proxy size to something even smaller than the built-in 256 px.
//!
//! All fields are optional in the on-disk file; missing fields fall back to
//! their built-in defaults. The loader is intentionally tolerant of malformed
//! input — any error simply yields the defaults so a broken config file can
//! never prevent the app from starting.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Default preview-resolution tiers (max pixels on the long edge).
///
/// Index 1 (512 px) is the historical default.
pub const DEFAULT_PROXY_RESOLUTIONS: &[u32] = &[256, 512, 768, 1024];

/// Lower bound for any user-supplied proxy resolution (px on the long edge).
const MIN_PROXY_RESOLUTION: u32 = 64;

/// Upper bound for any user-supplied proxy resolution (px on the long edge).
///
/// 16 384 px is plenty for live preview on any practical terminal and keeps
/// memory usage bounded even on misconfigured systems.
const MAX_PROXY_RESOLUTION: u32 = 16_384;

/// Raw, on-disk schema. Every field is `Option` so unspecified entries simply
/// stay at their built-in defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfigFile {
    /// Override for the preview-resolution tiers cycled with `[` / `]`.
    /// Values are clamped to `[64, 16384]`, deduplicated, and sorted on load.
    /// An empty or all-invalid list silently falls back to the defaults.
    pub proxy_resolutions: Option<Vec<u32>>,
}

/// Resolved application configuration with defaults applied.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Preview-resolution tiers (px on the long edge), sorted ascending,
    /// guaranteed to be non-empty.
    pub proxy_resolutions: Vec<u32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            proxy_resolutions: DEFAULT_PROXY_RESOLUTIONS.to_vec(),
        }
    }
}

impl AppConfig {
    /// Default on-disk path: `~/.config/spix/config.json`.
    pub fn config_path() -> Option<PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("spix");
        path.push("config.json");
        Some(path)
    }

    /// Load from the default path, falling back to defaults on any error.
    pub fn load_default() -> Self {
        Self::config_path()
            .map(|p| Self::load_from_path(&p))
            .unwrap_or_default()
    }

    /// Load from an explicit path. Returns defaults if the file is missing,
    /// unreadable, or contains no valid overrides.
    pub fn load_from_path(path: &std::path::Path) -> Self {
        let mut cfg = Self::default();
        if let Ok(data) = crate::config::read_to_string_limited(path, 1_000_000)
            && let Ok(file) = serde_json::from_str::<AppConfigFile>(&data)
            && let Some(list) = file.proxy_resolutions
            && let Some(sanitized) = sanitize_proxy_resolutions(&list)
        {
            cfg.proxy_resolutions = sanitized;
        }
        cfg
    }
}

/// Clamp, dedupe, and sort a user-supplied resolution list.
///
/// Returns `None` if every value was out of range, in which case the caller
/// should keep the defaults.
fn sanitize_proxy_resolutions(values: &[u32]) -> Option<Vec<u32>> {
    let mut cleaned: Vec<u32> = values
        .iter()
        .copied()
        .filter(|v| (MIN_PROXY_RESOLUTION..=MAX_PROXY_RESOLUTION).contains(v))
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.sort_unstable();
    cleaned.dedup();
    Some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_built_in() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.proxy_resolutions, DEFAULT_PROXY_RESOLUTIONS);
    }

    #[test]
    fn sanitize_clamps_and_sorts() {
        let cleaned = sanitize_proxy_resolutions(&[1024, 0, 256, 512, 30, 99_999, 256]).unwrap();
        // 0 and 30 (< MIN) and 99_999 (> MAX) dropped, duplicate 256 deduped, sorted.
        assert_eq!(cleaned, vec![256, 512, 1024]);
    }

    #[test]
    fn sanitize_rejects_all_invalid() {
        assert!(sanitize_proxy_resolutions(&[0, 1, 99_999]).is_none());
        assert!(sanitize_proxy_resolutions(&[]).is_none());
    }

    #[test]
    fn load_from_missing_file_returns_defaults() {
        let cfg = AppConfig::load_from_path(std::path::Path::new(
            "/definitely/does/not/exist/spix-config.json",
        ));
        assert_eq!(cfg.proxy_resolutions, DEFAULT_PROXY_RESOLUTIONS);
    }

    #[test]
    fn load_from_valid_file_overrides() {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"proxy_resolutions": [128, 384, 1024, 2048]}}"#).unwrap();
        let cfg = AppConfig::load_from_path(file.path());
        assert_eq!(cfg.proxy_resolutions, vec![128, 384, 1024, 2048]);
    }

    #[test]
    fn load_from_malformed_file_returns_defaults() {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "not json {{").unwrap();
        let cfg = AppConfig::load_from_path(file.path());
        assert_eq!(cfg.proxy_resolutions, DEFAULT_PROXY_RESOLUTIONS);
    }
}
