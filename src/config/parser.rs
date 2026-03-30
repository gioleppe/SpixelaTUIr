use anyhow::{Context, Result};
use std::path::Path;

use crate::effects::Pipeline;

/// Load a [`Pipeline`] from a YAML or JSON file.
///
/// The file format is inferred from the file extension:
/// * `.yaml` / `.yml` → YAML (via `serde_yml`)
/// * `.json` → JSON (via `serde_json`)
///
/// Files with no recognised extension are tried as YAML first, then JSON.
pub fn load_pipeline(path: &Path) -> Result<Pipeline> {
    log::info!("Loading pipeline from {}", path.display());
    let contents = crate::config::read_to_string_limited(path, 10_000_000)
        .with_context(|| format!("reading {}", path.display()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    match ext.as_deref() {
        Some("json") => {
            let pipeline: Pipeline = serde_json::from_str(&contents)
                .with_context(|| format!("parsing JSON from {}", path.display()))?;
            log::info!("Loaded {} effects from JSON", pipeline.effects.len());
            Ok(pipeline)
        }
        _ => {
            let pipeline: Pipeline = serde_yml::from_str(&contents)
                .with_context(|| format!("parsing YAML from {}", path.display()))?;
            log::info!("Loaded {} effects from YAML", pipeline.effects.len());
            Ok(pipeline)
        }
    }
}

/// Serialize a [`Pipeline`] to a pretty-printed JSON string.
pub fn serialize_pipeline(pipeline: &Pipeline) -> Result<String> {
    let json = serde_json::to_string_pretty(pipeline)?;
    Ok(json)
}

/// Write a [`Pipeline`] to a JSON file at `path`.
///
/// The file is written as pretty-printed JSON regardless of the file
/// extension supplied by the caller.  Parent directories are **not** created
/// automatically.
pub fn save_pipeline(pipeline: &Pipeline, path: &Path) -> Result<()> {
    log::info!(
        "Saving pipeline ({} effects) to {}",
        pipeline.effects.len(),
        path.display()
    );
    let json = serialize_pipeline(pipeline)?;
    std::fs::write(path, json)
        .with_context(|| format!("writing pipeline to {}", path.display()))?;
    log::debug!("Pipeline saved successfully");
    Ok(())
}
