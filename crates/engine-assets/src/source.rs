use std::collections::HashMap;
use std::path::PathBuf;

use crate::AssetError;

/// Where raw asset bytes come from. Development uses the filesystem; release builds are expected
/// to plug in a packed bundle reader; tests use memory. Implementations must be thread-safe so
/// loading can move to worker threads later.
pub trait AssetSource: Send + Sync {
    fn read(&self, name: &str) -> Result<Vec<u8>, AssetError>;
}

pub struct FileSystemSource {
    root: PathBuf,
}

impl FileSystemSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetSource for FileSystemSource {
    fn read(&self, name: &str) -> Result<Vec<u8>, AssetError> {
        // Asset names are logical paths; refuse anything that could escape the root.
        if name.split(['/', '\\']).any(|c| c == "..") || name.starts_with(['/', '\\']) {
            return Err(AssetError::NotFound(name.to_owned()));
        }
        std::fs::read(self.root.join(name)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AssetError::NotFound(name.to_owned()),
            _ => AssetError::Io(e),
        })
    }
}

#[derive(Default)]
pub struct MemorySource {
    files: HashMap<String, Vec<u8>>,
}

impl MemorySource {
    pub fn with(mut self, name: impl Into<String>, bytes: Vec<u8>) -> Self {
        self.files.insert(name.into(), bytes);
        self
    }
}

impl AssetSource for MemorySource {
    fn read(&self, name: &str) -> Result<Vec<u8>, AssetError> {
        self.files.get(name).cloned().ok_or_else(|| AssetError::NotFound(name.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_source_rejects_traversal() {
        let s = FileSystemSource::new(".");
        assert!(matches!(s.read("../secret"), Err(AssetError::NotFound(_))));
        assert!(matches!(s.read("/etc/passwd"), Err(AssetError::NotFound(_))));
        assert!(matches!(s.read("definitely/missing.png"), Err(AssetError::NotFound(_))));
    }
}
