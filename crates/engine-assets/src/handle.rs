use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use crate::AssetError;

/// Stable identity of an asset: a 64-bit FNV-1a hash of its logical name.
///
/// Hashing the name (instead of handing out incrementing indices) means ids are identical across
/// runs and tools, so a future bundler can store them on disk, and components can reference assets
/// before they are loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub u64);

impl AssetId {
    pub const fn from_name(name: &str) -> Self {
        let bytes = name.as_bytes();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut i = 0;
        while i < bytes.len() {
            h ^= bytes[i] as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            i += 1;
        }
        AssetId(h)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Texture,
    Sprite,
    Mesh,
    Material,
    Skeleton,
    Animation,
    Map,
}

/// A type that can live in the [`crate::AssetManager`].
pub trait Asset: Send + Sync + Sized + 'static {
    const KIND: AssetKind;

    /// Decodes the asset from raw source bytes. Types that are only constructed in code keep the
    /// default.
    fn decode(_bytes: &[u8]) -> Result<Self, AssetError> {
        Err(AssetError::Unsupported(Self::KIND))
    }
}

/// Typed, copyable reference to an asset. Cheap to store in components; resolving it to data is
/// the job of the [`crate::AssetManager`], and may fail (not loaded yet, failed, unknown).
pub struct AssetHandle<T> {
    id: AssetId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> AssetHandle<T> {
    pub const fn from_id(id: AssetId) -> Self {
        Self { id, _marker: PhantomData }
    }
    pub const fn from_name(name: &str) -> Self {
        Self::from_id(AssetId::from_name(name))
    }
    pub const fn id(&self) -> AssetId {
        self.id
    }
}

impl<T> Clone for AssetHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for AssetHandle<T> {}
impl<T> PartialEq for AssetHandle<T> {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id
    }
}
impl<T> Eq for AssetHandle<T> {}
impl<T> Hash for AssetHandle<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h)
    }
}
impl<T> core::fmt::Debug for AssetHandle<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AssetHandle<{}>({:016x})", core::any::type_name::<T>().rsplit("::").next().unwrap_or("?"), self.id.0)
    }
}
impl<T> From<&str> for AssetHandle<T> {
    fn from(name: &str) -> Self {
        Self::from_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_distinct() {
        const A: AssetId = AssetId::from_name("player");
        assert_eq!(A, AssetId::from_name("player"));
        assert_ne!(A, AssetId::from_name("player2"));
        // Pinned so bundles written by one build can be read by another.
        assert_eq!(AssetId::from_name("").0, 0xcbf2_9ce4_8422_2325);
        assert_eq!(AssetId::from_name("a").0, 0xaf63_dc4c_8601_ec8c);
    }
}
