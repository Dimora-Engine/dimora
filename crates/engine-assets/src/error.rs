use crate::AssetKind;

#[derive(Debug)]
pub enum AssetError {
    NotFound(String),
    Io(std::io::Error),
    Decode(String),
    /// The asset type has no byte-level decoder (it is created in code or by a future bundler).
    Unsupported(AssetKind),
}

impl core::fmt::Display for AssetError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AssetError::NotFound(n) => write!(f, "asset `{n}` not found"),
            AssetError::Io(e) => write!(f, "i/o error: {e}"),
            AssetError::Decode(e) => write!(f, "decode error: {e}"),
            AssetError::Unsupported(k) => write!(f, "{k:?} assets cannot be loaded from bytes yet"),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<std::io::Error> for AssetError {
    fn from(e: std::io::Error) -> Self {
        AssetError::Io(e)
    }
}
