//! CPU descriptions of offscreen color targets.
//!
//! The store lives on the world and holds no GPU objects. [`crate::extract_scene`] copies the
//! descriptions into [`crate::RenderScene`]. The wgpu backend allocates the color image, its
//! view, and a private depth image, and it keeps them until the description is removed or resized.
//!
//! A target is not an [`engine_assets::AssetId`] and it is not loaded from disk. Ids start at 1
//! and are never reused, so a removed id cannot name a later target.
//!
//! Color images use the renderer's existing RGBA8 attachment format (`Rgba8UnormSrgb` when the
//! surface is sRGB, `Rgba8Unorm` when the renderer was created linear). Depth is `Depth32Float`
//! and is not sampleable. There is no format list, HDR target, or MSAA resolve.
//!
//! Sampling is frame-local. A sprite may sample a target only after an earlier camera in this
//! frame has written it, and not while the sprite's own camera is writing that same target.
//! A missing producer does not fall back to the previous frame's pixels. Camera order is not
//! rearranged to satisfy a dependency.

use std::cell::RefCell;
use std::collections::HashMap;

/// Largest width or height [`RenderTargetStore`] accepts.
///
/// This is wgpu 30.0.1's default `max_texture_dimension_2d`. The backend also refuses a size the
/// live device cannot allocate. A tighter device limit is not a second public format negotiation.
pub const RENDER_TARGET_MAX_EXTENT: u32 = 8192;

/// How many distinct target ids extraction remembers after logging a fault.
const WARNING_CAPACITY: usize = 64;

/// Identity of one offscreen target.
///
/// This is not a texture asset id. Equality is the id assigned at creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderTargetId(u64);

impl RenderTargetId {
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for RenderTargetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "render-target-{}", self.0)
    }
}

/// Plain description of one live target. The renderer owns the GPU image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderTargetDesc {
    pub id: RenderTargetId,
    pub width: u32,
    pub height: u32,
    /// Increments when [`RenderTargetStore::resize`] changes the dimensions. An unchanged size
    /// keeps this value. The GPU image is replaced when it changes. Pixels in the new image are
    /// undefined until a camera writes the target in that frame. Old pixels are not copied.
    pub generation: u64,
}

/// Why [`RenderTargetStore`] rejected a size or a missing id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTargetError {
    /// Width or height is zero.
    ZeroDimension { width: u32, height: u32 },
    /// An axis is above [`RENDER_TARGET_MAX_EXTENT`].
    Extent { width: u32, height: u32, limit: u32 },
    /// `width * height * 4` does not fit in `u64`.
    Allocation { width: u32, height: u32 },
    /// `resize` or a lookup named an id that is not in the store.
    Missing(RenderTargetId),
}

impl std::fmt::Display for RenderTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroDimension { width, height } => {
                write!(f, "render target size {width}x{height} must be at least 1x1")
            }
            Self::Extent { width, height, limit } => {
                write!(f, "render target size {width}x{height} exceeds the {limit} extent limit")
            }
            Self::Allocation { width, height } => {
                write!(f, "render target size {width}x{height} overflows the byte-size calculation")
            }
            Self::Missing(id) => write!(f, "render target {id} is not in the store"),
        }
    }
}

impl std::error::Error for RenderTargetError {}

/// Rejects a size the store will not record.
///
/// Zero is rejected before the extent cap. The byte-size check uses `u64` multiplication and
/// rejects an overflow. The extent cap is [`RENDER_TARGET_MAX_EXTENT`] on each axis.
pub fn check_render_target_size(width: u32, height: u32) -> Result<u64, RenderTargetError> {
    if width == 0 || height == 0 {
        return Err(RenderTargetError::ZeroDimension { width, height });
    }
    let Some(pixels) = u64::from(width).checked_mul(u64::from(height)) else {
        return Err(RenderTargetError::Allocation { width, height });
    };
    let Some(bytes) = pixels.checked_mul(4) else {
        return Err(RenderTargetError::Allocation { width, height });
    };
    if width > RENDER_TARGET_MAX_EXTENT || height > RENDER_TARGET_MAX_EXTENT {
        return Err(RenderTargetError::Extent { width, height, limit: RENDER_TARGET_MAX_EXTENT });
    }
    Ok(bytes)
}

/// World resource of offscreen target descriptions.
///
/// Creating, resizing, and removing are explicit. Removing a camera does not remove a target.
/// The store does not own wgpu textures, views, or samplers, and it does not talk to
/// [`engine_assets::AssetManager`].
#[derive(Debug)]
pub struct RenderTargetStore {
    next: u64,
    targets: HashMap<RenderTargetId, RenderTargetDesc>,
    warned: RefCell<Vec<RenderTargetId>>,
}

impl Default for RenderTargetStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderTargetStore {
    pub fn new() -> Self {
        Self { next: 1, targets: HashMap::new(), warned: RefCell::new(Vec::new()) }
    }

    pub fn create(&mut self, width: u32, height: u32) -> Result<RenderTargetId, RenderTargetError> {
        check_render_target_size(width, height)?;
        let id = RenderTargetId(self.next);
        let Some(next) = self.next.checked_add(1) else {
            return Err(RenderTargetError::Allocation { width, height });
        };
        self.next = next;
        self.targets.insert(id, RenderTargetDesc { id, width, height, generation: 1 });
        Ok(id)
    }

    pub fn get(&self, id: RenderTargetId) -> Option<&RenderTargetDesc> {
        self.targets.get(&id)
    }

    /// Changes the recorded size. The same id is kept.
    ///
    /// An equal size leaves [`RenderTargetDesc::generation`] unchanged and allocates nothing.
    /// A different size increments the generation. The GPU image is replaced before the next
    /// frame's passes, not while a pass has the image attached. Pixels of the replacement are
    /// undefined until a camera writes the target.
    pub fn resize(&mut self, id: RenderTargetId, width: u32, height: u32) -> Result<(), RenderTargetError> {
        check_render_target_size(width, height)?;
        let Some(desc) = self.targets.get_mut(&id) else {
            return Err(RenderTargetError::Missing(id));
        };
        if desc.width == width && desc.height == height {
            return Ok(());
        }
        desc.width = width;
        desc.height = height;
        desc.generation = desc.generation.saturating_add(1);
        Ok(())
    }

    /// Drops the description. Returns whether it was present. The id is not issued again.
    /// The renderer drops the GPU image on the next extract that no longer lists it.
    pub fn remove(&mut self, id: RenderTargetId) -> bool {
        self.targets.remove(&id).is_some()
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Live descriptions, sorted by id so a frame's copy does not depend on hash order.
    pub fn descriptions(&self) -> Vec<RenderTargetDesc> {
        let mut list: Vec<_> = self.targets.values().copied().collect();
        list.sort_by_key(|desc| desc.id);
        list
    }

    /// `true` the first time `id` is reported. Later calls stay quiet so a missing target does
    /// not log on every sprite of every frame. The buffer does not grow past 64 ids.
    pub(crate) fn warn_once(&self, id: RenderTargetId) -> bool {
        let mut warned = self.warned.borrow_mut();
        if warned.contains(&id) || warned.len() == WARNING_CAPACITY {
            return false;
        }
        warned.push(id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_fault_is_logged_once_until_the_buffer_is_full() {
        let mut store = RenderTargetStore::new();
        let first = store.create(1, 1).unwrap();
        assert!(store.warn_once(first));
        assert!(!store.warn_once(first));
        for _ in 1..64 {
            let id = store.create(1, 1).unwrap();
            assert!(store.warn_once(id));
        }
        let extra = store.create(1, 1).unwrap();
        assert!(!store.warn_once(extra), "a full buffer stays quiet instead of logging every frame");
        assert!(!store.warn_once(first));
    }
}
