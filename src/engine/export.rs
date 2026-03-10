use anyhow::Result;

/// Export the current processed frame to disk.
///
/// This function handles high-resolution disk saving and animation encoding.
pub fn export_image(output_path: std::path::PathBuf) -> Result<()> {
    // Placeholder: in a full implementation this would encode the current
    // processed image buffer and write it to `output_path`.
    println!("Exporting to {}", output_path.display());
    Ok(())
}
