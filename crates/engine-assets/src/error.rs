use crate::AssetKind;

#[derive(Debug)]
pub enum AssetError {
    NotFound(String),
    Io(std::io::Error),
    Decode(String),
    /// The asset type has no byte-level decoder (it is created in code or by a future bundler).
    Unsupported(AssetKind),
    /// Two different names hash to the same [`crate::AssetId`] for the same asset type. The asset
    /// already stored under `existing` is left as it was; rename one of the two.
    IdCollision {
        id: crate::AssetId,
        existing: String,
        new: String,
    },
}

impl core::fmt::Display for AssetError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AssetError::NotFound(n) => write!(f, "asset `{n}` not found"),
            AssetError::Io(e) => write!(f, "i/o error: {e}"),
            AssetError::Decode(e) => write!(f, "decode error: {e}"),
            AssetError::Unsupported(k) => write!(f, "{k:?} assets cannot be loaded from bytes yet"),
            AssetError::IdCollision { id, existing, new } => {
                write!(f, "asset names `{existing}` and `{new}` hash to the same id {:#018x}; rename one of them", id.0)
            }
        }
    }
}

impl std::error::Error for AssetError {}

impl From<std::io::Error> for AssetError {
    fn from(e: std::io::Error) -> Self {
        AssetError::Io(e)
    }
}
