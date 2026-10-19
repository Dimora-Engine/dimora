//! One warning per missing renderable asset, with a fixed-size memory of ids already reported.

use std::cell::{Cell, RefCell};

use engine_assets::{Asset, AssetHandle, AssetId, AssetManager, LoadState, Texture};
use engine_world::World;

use crate::scene::ExtractStats;

/// How many distinct missing assets extraction will remember.
///
/// The buffer holds at most this many ids. It does not grow and it does not rotate. A miss whose id
/// is already stored is not logged again. A distinct miss after the buffer is full still increments
/// [`ExtractStats::missing_assets`] on that extraction, and it is not logged individually. The first
/// such drop logs one aggregate line. Later drops stay silent until a remembered asset loads, its id
/// is forgotten, and the buffer is no longer full. A pending load is not stored and does not log.
/// Without this resource on the world, every miss logs, because there is no memory to deduplicate.
pub const MISSING_ASSET_WARNING_CAPACITY: usize = 64;

/// Ids of renderable assets that extraction has already logged as missing, failed, or invalid.
///
/// [`crate::extract_scene`] uses this when it is present on the world. `Game::new` inserts one.
/// A pending load is not recorded and does not log. After the asset is loaded and extracted, its
/// id is forgotten, so a later miss logs again.
///
/// The buffer is interior-mutable so extraction can update it through a shared [`World`] borrow.
/// `overflow_reported` is one flag, not another id list.
#[derive(Default)]
pub struct MissingAssetWarnings {
    recent: RefCell<Vec<AssetId>>,
    overflow_reported: Cell<bool>,
}

enum WarningSlot {
    /// `id` was stored. The caller logs this asset.
    Fresh,
    /// `id` is already stored. The caller does not log.
    Repeat,
    /// The buffer is full and `id` was not stored. The caller does not log this asset.
    Suppressed,
}

impl MissingAssetWarnings {
    pub(crate) fn is_empty(&self) -> bool {
        self.recent.borrow().is_empty()
    }

    fn track(&self, id: AssetId) -> WarningSlot {
        let mut recent = self.recent.borrow_mut();
        if recent.contains(&id) {
            return WarningSlot::Repeat;
        }
        if recent.len() == MISSING_ASSET_WARNING_CAPACITY {
            return WarningSlot::Suppressed;
        }
        recent.push(id);
        WarningSlot::Fresh
    }

    /// `true` once per time the buffer fills and then drops a new id.
    fn note_suppression(&self) -> bool {
        if self.overflow_reported.get() {
            return false;
        }
        self.overflow_reported.set(true);
        true
    }

    /// Drops `id` because the asset resolved. Returns whether any ids are still remembered.
    pub(crate) fn resolved(&self, id: AssetId) -> bool {
        let mut recent = self.recent.borrow_mut();
        if let Some(index) = recent.iter().position(|existing| *existing == id) {
            recent.swap_remove(index);
        }
        if recent.len() < MISSING_ASSET_WARNING_CAPACITY {
            self.overflow_reported.set(false);
        }
        !recent.is_empty()
    }
}

fn remember(world: &World, id: AssetId, watch: &mut bool) -> bool {
    let Some(warnings) = world.try_resource::<MissingAssetWarnings>() else {
        return true;
    };
    match warnings.track(id) {
        WarningSlot::Fresh => {
            *watch = true;
            true
        }
        WarningSlot::Repeat => false,
        WarningSlot::Suppressed => {
            if warnings.note_suppression() {
                log::warn!(
                    "missing-asset warning buffer is full ({MISSING_ASSET_WARNING_CAPACITY}); further distinct misses are counted and not logged individually until a remembered asset loads"
                );
            }
            false
        }
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

/// Reports a texture referenced by a sprite or mesh that is otherwise present.
///
/// The draw is not skipped. A pending load does not log. A name is included only when
/// [`AssetManager::name_of`] returns one. [`AssetHandle`] does not store the name, so a handle that
/// was never registered, or whose entry was removed, is logged as its id. `owner` is the parent
/// asset's kind and real name when that entry exists. It is not used as a texture name.
pub(crate) fn report_absent_texture(
    world: &World,
    assets: &AssetManager,
    handle: AssetHandle<Texture>,
    kind: &str,
    owner: Option<(&str, &str)>,
    watch: &mut bool,
    stats: &mut ExtractStats,
) {
    if assets.get(handle).is_some() {
        if *watch {
            if let Some(warnings) = world.try_resource::<MissingAssetWarnings>() {
                *watch = warnings.resolved(handle.id());
            }
        }
        return;
    }
    let reason = match assets.state(handle) {
        LoadState::Loading => return,
        LoadState::Failed => "failed to load",
        LoadState::Loaded => "is invalid",
        LoadState::Unknown => "is missing",
    };
    stats.missing_assets += 1;
    let id = handle.id();
    if !remember(world, id, watch) {
        return;
    }
    match assets.name_of(handle) {
        Some(name) => log::warn!("missing {kind}: asset '{name}' {reason}"),
        None => match owner {
            Some((owner_kind, owner)) => {
                log::warn!("missing {kind}: asset {:016x} {reason}; referenced by {owner_kind} '{owner}'", id.0)
            }
            None => log::warn!("missing {kind}: asset {:016x} {reason}", id.0),
        },
    }
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
    if !remember(world, id, watch) {
        return;
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
            assert!(matches!(warnings.track(*id), WarningSlot::Fresh));
        }
        let extra = AssetId(1_000);
        assert!(matches!(warnings.track(extra), WarningSlot::Suppressed), "a full buffer does not grow or rotate");
        assert!(warnings.note_suppression());
        assert!(!warnings.note_suppression(), "the aggregate is logged once per fill");
        assert!(matches!(warnings.track(ids[0]), WarningSlot::Repeat), "a remembered id is not logged again");
        assert!(warnings.resolved(ids[0]), "forgetting one id leaves the rest remembered");
        assert!(matches!(warnings.track(ids[0]), WarningSlot::Fresh), "a resolved id can be tracked again");
        assert!(matches!(warnings.track(extra), WarningSlot::Suppressed), "the buffer is full again");
        assert!(warnings.note_suppression(), "freeing a slot allows one later aggregate");

        let one = MissingAssetWarnings::default();
        assert!(matches!(one.track(AssetId(1)), WarningSlot::Fresh));
        assert!(!one.resolved(AssetId(1)));
        assert!(one.is_empty());
    }
}
