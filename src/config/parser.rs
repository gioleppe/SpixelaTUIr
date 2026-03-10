use anyhow::Result;
use std::path::Path;

use crate::effects::Pipeline;

/// Load a [`Pipeline`] from a YAML or JSON file.
///
/// The file format is inferred from the file extension:
/// * `.yaml` / `.yml` → YAML
/// * `.json` → JSON (via serde_yaml's JSON compatibility layer)
pub fn load_pipeline(path: &Path) -> Result<Pipeline> {
    let contents = std::fs::read_to_string(path)?;
    let pipeline: Pipeline = serde_yml::from_str(&contents)?;
    Ok(pipeline)
}

/// Serialize a [`Pipeline`] to a YAML string.
pub fn serialize_pipeline(pipeline: &Pipeline) -> Result<String> {
    let yaml = serde_yml::to_string(pipeline)?;
    Ok(yaml)
}
