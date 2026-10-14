//! One warning per missing renderable asset, with a fixed-size memory of ids already reported.

use std::cell::RefCell;

use engine_assets::{Asset, AssetHandle, AssetId, AssetManager, LoadState};
use engine_world::World;

use crate::scene::ExtractStats;

/// How many distinct missing assets extraction will remember.
///
/// Past this many, further distinct misses stay in [`ExtractStats::missing_assets`] and are not
/// logged until a remembered asset loads and frees a slot. The memory does not grow with the
/// world, and a full buffer does not rotate: rotating would warn again on every frame.
pub const MISSING_ASSET_WARNING_CAPACITY: usize = 64;

/// Ids of renderable assets that extraction has already logged as missing, failed, or invalid.
///
/// [`crate::extract_scene`] uses this when it is present on the world. `Game::new` inserts one.
/// A pending load is not recorded and does not log. After the asset is loaded and extracted, its
/// id is forgotten, so a later miss logs again.
///
/// The buffer is interior-mutable so extraction can update it through a shared [`World`] borrow.
#[derive(Default)]
pub struct MissingAssetWarnings {
    recent: RefCell<Vec<AssetId>>,
}

impl MissingAssetWarnings {
    pub(crate) fn is_empty(&self) -> bool {
        self.recent.borrow().is_empty()
    }

    /// `true` when `id` was newly remembered and the caller should log.
    pub(crate) fn track(&self, id: AssetId) -> bool {
        let mut recent = self.recent.borrow_mut();
        if recent.contains(&id) || recent.len() == MISSING_ASSET_WARNING_CAPACITY {
            return false;
        }
        recent.push(id);
        true
    }

    /// Drops `id` because the asset resolved. Returns whether any ids are still remembered.
    pub(crate) fn resolved(&self, id: AssetId) -> bool {
        let mut recent = self.recent.borrow_mut();
        if let Some(index) = recent.iter().position(|existing| *existing == id) {
            recent.swap_remove(index);
        }
        !recent.is_empty()
    }
}

pub(crate) fn resolve<'a, T: Asset>(
    world: &World,
    assets: &'a AssetManager,
    handle: AssetHandle<T>,
    kind: &str,
    watch: &mut bool,
    stats: &mut ExtractStats,
) -> Option<&'a T> {
    if let Some(value) = assets.get(handle) {
        if *watch {
            if let Some(warnings) = world.try_resource::<MissingAssetWarnings>() {
                *watch = warnings.resolved(handle.id());
            }
        }
        return Some(value);
    }
    stats.missing_assets += 1;
    let reason = match assets.state(handle) {
        // A requested load has not been processed. That is not a failure.
        LoadState::Loading => return None,
        LoadState::Failed => "failed to load",
        LoadState::Loaded => "is invalid",
        LoadState::Unknown => "is missing",
    };
    note(world, assets, handle, kind, reason, watch);
    None
}

fn note<T: Asset>(
    world: &World,
    assets: &AssetManager,
    handle: AssetHandle<T>,
    kind: &str,
    reason: &str,
    watch: &mut bool,
) {
    let id = handle.id();
    if let Some(warnings) = world.try_resource::<MissingAssetWarnings>() {
        if !warnings.track(id) {
            return;
        }
        *watch = true;
    }
    match assets.name_of(handle) {
        Some(name) => log::warn!("skipping {kind}: asset '{name}' {reason}"),
        None => log::warn!("skipping {kind}: asset {:016x} {reason}", id.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_is_bounded_and_a_resolved_id_can_be_tracked_again() {
        let warnings = MissingAssetWarnings::default();
        let ids: Vec<AssetId> = (0..MISSING_ASSET_WARNING_CAPACITY as u64).map(AssetId).collect();
        for id in &ids {
            assert!(warnings.track(*id));
        }
        let extra = AssetId(1_000);
        assert!(!warnings.track(extra), "a full buffer does not grow or rotate");
        assert!(!warnings.track(ids[0]), "a remembered id is not logged again");
        assert!(warnings.resolved(ids[0]), "forgetting one id leaves the rest remembered");
        assert!(warnings.track(ids[0]), "a resolved id can be tracked again");
        assert!(!warnings.track(extra), "the buffer is full again");

        let one = MissingAssetWarnings::default();
        assert!(one.track(AssetId(1)));
        assert!(!one.resolved(AssetId(1)));
        assert!(one.is_empty());
    }
}
