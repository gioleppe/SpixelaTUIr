pub mod favorites;
pub mod parser;
pub mod theme;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Reads a file into a string with a maximum size limit.
/// Returns an error if the file exceeds the limit.
pub fn read_to_string_limited<P: AsRef<Path>>(path: P, limit: u64) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    // Try to read up to limit + 1 bytes
    let n = file.take(limit + 1).read_to_end(&mut buffer)?;
    if n > limit as usize {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("File exceeds size limit of {} bytes", limit),
        ));
    }
    String::from_utf8(buffer).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_to_string_limited_ok() -> io::Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "Hello, world!")?;
        let content = read_to_string_limited(file.path(), 100)?;
        assert_eq!(content, "Hello, world!\n");
        Ok(())
    }

    #[test]
    fn test_read_to_string_limited_too_large() -> io::Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "This is a long sentence that should exceed the limit.")?;
        let result = read_to_string_limited(file.path(), 10);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("File exceeds size limit"));
        Ok(())
    }
}
