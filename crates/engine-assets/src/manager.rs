use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    Asset, AssetError, AssetHandle, AssetId, AssetSource, FileSystemSource, MaterialAsset, MemorySource, MeshData,
    SpriteAsset, Texture,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadState {
    /// Nothing is known about this id.
    Unknown,
    /// A load was requested and has not been processed yet.
    Loading,
    Loaded,
    Failed,
}

struct Entry<T> {
    name: String,
    state: LoadState,
    value: Option<Arc<T>>,
    /// A version stamp, 0 until a value exists. It changes on every insert or replace and never repeats
    /// for the same id, not even across [`AssetManager::remove`] (see `Store::next_revision`). GPU
    /// caches key on `(id, revision)`, which is what makes hot reload possible without the renderer
    /// knowing about files.
    revision: u32,
    dependencies: Vec<AssetId>,
    error: Option<String>,
}

struct Store<T> {
    entries: HashMap<AssetId, Entry<T>>,
    /// Last revision handed out by this store. One counter for the whole store, not per entry, so an
    /// asset that is removed and inserted again under the same name gets a revision no cache has seen.
    next_revision: u32,
}

/// What the manager needs from a store without knowing its asset type.
trait ErasedStore {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn contains(&self, id: AssetId) -> bool;
    fn forget_dependency(&mut self, id: AssetId);
}

impl<T: Asset> ErasedStore for Store<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn contains(&self, id: AssetId) -> bool {
        self.entries.contains_key(&id)
    }
    fn forget_dependency(&mut self, id: AssetId) {
        for entry in self.entries.values_mut() {
            entry.dependencies.retain(|d| *d != id);
        }
    }
}

struct PendingLoad {
    key: (TypeId, AssetId),
    run: Box<dyn FnOnce(&mut AssetManager) + Send>,
}

/// Owns all loaded assets of every type and the [`AssetSource`] they are read from.
///
/// Access is by typed [`AssetHandle`]; names are hashed to ids, so handles can be created before
/// anything is loaded. Loading is request/process: [`AssetManager::load`] records a request and
/// returns a handle immediately; [`AssetManager::process_loads`] performs the I/O and decoding.
/// Today that runs synchronously on the caller's thread. It is the single seam where a worker
/// pool plugs in later (the source is `Send + Sync` and decoders are plain functions).
pub struct AssetManager {
    source: Arc<dyn AssetSource>,
    stores: HashMap<TypeId, Box<dyn ErasedStore>>,
    pending: Vec<PendingLoad>,
}

impl AssetManager {
    pub fn new(source: impl AssetSource + 'static) -> Self {
        Self { source: Arc::new(source), stores: HashMap::new(), pending: Vec::new() }
    }

    /// Development setup: assets are files under `root`.
    pub fn filesystem(root: impl Into<PathBuf>) -> Self {
        Self::new(FileSystemSource::new(root))
    }

    /// No external files; everything is inserted from code.
    pub fn in_memory() -> Self {
        Self::new(MemorySource::default())
    }

    fn store<T: Asset>(&self) -> Option<&Store<T>> {
        self.stores.get(&TypeId::of::<T>()).and_then(|s| s.as_any().downcast_ref())
    }

    fn store_mut<T: Asset>(&mut self) -> &mut Store<T> {
        self.stores
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Store::<T> { entries: HashMap::new(), next_revision: 0 }))
            .as_any_mut()
            .downcast_mut()
            .expect("store type matches its TypeId")
    }

    fn entry<T: Asset>(&self, id: AssetId) -> Option<&Entry<T>> {
        self.store::<T>()?.entries.get(&id)
    }

    // ---- handles ---------------------------------------------------------------------------

    /// Handle for an asset by name. Has no side effects; the asset may not exist (yet).
    pub fn handle<T: Asset>(&self, name: &str) -> AssetHandle<T> {
        AssetHandle::from_name(name)
    }
    pub fn texture(&self, name: &str) -> AssetHandle<Texture> {
        self.handle(name)
    }
    pub fn sprite(&self, name: &str) -> AssetHandle<SpriteAsset> {
        self.handle(name)
    }
    pub fn mesh(&self, name: &str) -> AssetHandle<MeshData> {
        self.handle(name)
    }
    pub fn material(&self, name: &str) -> AssetHandle<MaterialAsset> {
        self.handle(name)
    }

    // ---- inserting and reading ---------------------------------------------------------------

    /// Stores `value` under `name`, replacing (and changing the revision of) any previous value of
    /// the same name.
    ///
    /// Ids are 64-bit hashes of names. If a *different* name already owns the same id for this asset
    /// type, nothing is stored and [`AssetError::IdCollision`] is returned: the existing asset is
    /// never overwritten.
    pub fn insert<T: Asset>(&mut self, name: &str, value: T) -> Result<AssetHandle<T>, AssetError> {
        self.insert_with_id(AssetId::from_name(name), name, value)
    }

    /// `insert` with the id chosen by the caller. Only the tests need this, to force a collision
    /// without searching for two real names that collide.
    fn insert_with_id<T: Asset>(&mut self, id: AssetId, name: &str, value: T) -> Result<AssetHandle<T>, AssetError> {
        self.check_collision::<T>(id, name)?;
        let store = self.store_mut::<T>();
        store.next_revision += 1;
        let revision = store.next_revision;
        let entry = store.entries.entry(id).or_insert_with(|| Entry {
            name: name.to_owned(),
            state: LoadState::Unknown,
            value: None,
            revision: 0,
            dependencies: Vec::new(),
            error: None,
        });
        entry.revision = revision;
        entry.state = LoadState::Loaded;
        entry.value = Some(Arc::new(value));
        entry.error = None;
        Ok(AssetHandle::from_id(id))
    }

    fn check_collision<T: Asset>(&self, id: AssetId, name: &str) -> Result<(), AssetError> {
        match self.entry::<T>(id) {
            Some(existing) if existing.name != name => {
                Err(AssetError::IdCollision { id, existing: existing.name.clone(), new: name.to_owned() })
            }
            _ => Ok(()),
        }
    }

    pub fn get<T: Asset>(&self, handle: AssetHandle<T>) -> Option<&T> {
        self.entry::<T>(handle.id())?.value.as_deref()
    }

    pub fn state<T: Asset>(&self, handle: AssetHandle<T>) -> LoadState {
        self.entry::<T>(handle.id()).map_or(LoadState::Unknown, |e| e.state)
    }

    /// Version stamp of the value behind `handle`: 0 if there is none, otherwise a number that changes
    /// whenever the value is replaced and is never reused for this id, even after [`AssetManager::remove`].
    /// Compare it for equality; do not read meaning into its size.
    pub fn revision<T: Asset>(&self, handle: AssetHandle<T>) -> u32 {
        self.entry::<T>(handle.id()).map_or(0, |e| e.revision)
    }

    pub fn name_of<T: Asset>(&self, handle: AssetHandle<T>) -> Option<&str> {
        self.entry::<T>(handle.id()).map(|e| e.name.as_str())
    }

    pub fn error_of<T: Asset>(&self, handle: AssetHandle<T>) -> Option<&str> {
        self.entry::<T>(handle.id())?.error.as_deref()
    }

    // ---- dependencies ------------------------------------------------------------------------

    /// Records that `parent` needs `child` (e.g. a sprite needs its texture). Used for tracking
    /// what must be loaded together and what to reload when a file changes.
    pub fn add_dependency<P: Asset, C: Asset>(&mut self, parent: AssetHandle<P>, child: AssetHandle<C>) {
        if let Some(e) = self.store_mut::<P>().entries.get_mut(&parent.id()) {
            if !e.dependencies.contains(&child.id()) {
                e.dependencies.push(child.id());
            }
        }
    }

    pub fn dependencies<T: Asset>(&self, handle: AssetHandle<T>) -> &[AssetId] {
        self.entry::<T>(handle.id()).map_or(&[], |e| e.dependencies.as_slice())
    }

    // ---- removing ----------------------------------------------------------------------------

    /// Forgets the asset behind `handle`: its value, state, error and revision, and its dependency
    /// links. Afterwards [`AssetManager::get`] returns `None`, [`AssetManager::state`] returns
    /// [`LoadState::Unknown`] and [`AssetManager::revision`] returns 0. A load that was requested
    /// but not yet processed is cancelled. Returns whether an entry existed.
    ///
    /// Dependency links are untyped ids. Links that other assets recorded to this one are dropped
    /// too, unless an asset of another type still exists under the same id (same name).
    /// Components that still hold the handle are fine: it simply stops resolving, like any handle
    /// to an asset that is not loaded. Renderers evict their GPU copies when they notice.
    pub fn remove<T: Asset>(&mut self, handle: AssetHandle<T>) -> bool {
        let id = handle.id();
        let type_id = TypeId::of::<T>();
        self.pending.retain(|job| job.key != (type_id, id));
        let removed = self
            .stores
            .get_mut(&type_id)
            .and_then(|s| s.as_any_mut().downcast_mut::<Store<T>>())
            .is_some_and(|store| store.entries.remove(&id).is_some());
        if removed && !self.stores.values().any(|s| s.contains(id)) {
            for store in self.stores.values_mut() {
                store.forget_dependency(id);
            }
        }
        removed
    }

    // ---- loading -----------------------------------------------------------------------------

    /// Requests that `name` be read from the source and decoded as `T`. Returns the handle
    /// immediately; call [`AssetManager::process_loads`] to perform the work. Already
    /// loaded/loading assets are left alone.
    ///
    /// Returns [`AssetError::IdCollision`], and requests nothing, if a different name already owns
    /// the same id for this asset type (see [`AssetManager::insert`]).
    pub fn load<T: Asset>(&mut self, name: &str) -> Result<AssetHandle<T>, AssetError> {
        self.load_with_id(AssetId::from_name(name), name)
    }

    fn load_with_id<T: Asset>(&mut self, id: AssetId, name: &str) -> Result<AssetHandle<T>, AssetError> {
        self.check_collision::<T>(id, name)?;
        if let Some(e) = self.entry::<T>(id) {
            if matches!(e.state, LoadState::Loaded | LoadState::Loading) {
                return Ok(AssetHandle::from_id(id));
            }
        }
        self.mark_loading::<T>(id, name);
        let name = name.to_owned();
        let run = Box::new(move |mgr: &mut AssetManager| {
            let result = mgr.source.read(&name).and_then(|bytes| T::decode(&bytes));
            match result {
                Ok(value) => {
                    // Cannot collide: the entry made by `mark_loading` owns the id, and `remove`
                    // cancels this job before anything else could take it.
                    let _ = mgr.insert_with_id(id, &name, value);
                }
                Err(err) => {
                    let message = err.to_string();
                    log::warn!("asset '{name}' failed to load: {message}");
                    let e = mgr.store_mut::<T>().entries.get_mut(&id).expect("entry created by mark_loading");
                    e.state = LoadState::Failed;
                    e.error = Some(message);
                }
            }
        });
        self.pending.push(PendingLoad { key: (TypeId::of::<T>(), id), run });
        Ok(AssetHandle::from_id(id))
    }

    fn mark_loading<T: Asset>(&mut self, id: AssetId, name: &str) {
        let e = self.store_mut::<T>().entries.entry(id).or_insert_with(|| Entry {
            name: name.to_owned(),
            state: LoadState::Unknown,
            value: None,
            revision: 0,
            dependencies: Vec::new(),
            error: None,
        });
        e.state = LoadState::Loading;
    }

    pub fn has_pending_loads(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Performs all requested loads. Returns how many were processed.
    pub fn process_loads(&mut self) -> usize {
        let pending = std::mem::take(&mut self.pending);
        let n = pending.len();
        for job in pending {
            (job.run)(self);
        }
        n
    }

    /// Re-reads `name` from the source and replaces the value, bumping its revision so renderers
    /// refresh their GPU copies. This is the building block for file-watcher hot reload.
    pub fn reload<T: Asset>(&mut self, name: &str) -> Result<AssetHandle<T>, AssetError> {
        self.check_collision::<T>(AssetId::from_name(name), name)?;
        let value = T::decode(&self.source.read(name)?)?;
        self.insert(name, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod capture {
        include!("../../log_capture.rs");
    }

    fn png_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        image::write_buffer_with_format(
            &mut std::io::Cursor::new(&mut out),
            &[255, 0, 0, 255, 0, 255, 0, 255],
            2,
            1,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        out
    }

    #[test]
    fn insert_get_and_revision() {
        let mut m = AssetManager::in_memory();
        let h = m.insert("t", Texture::solid(1, 1, [1, 2, 3, 4])).unwrap();
        assert_eq!(m.revision(h), 1);
        assert_eq!(m.get(h).unwrap().rgba, vec![1, 2, 3, 4]);
        m.insert("t", Texture::solid(1, 1, [9, 9, 9, 9])).unwrap();
        assert_eq!(m.revision(h), 2);
        assert_eq!(m.get(m.texture("t")).unwrap().rgba[0], 9);
        // Same name, different type: independent namespaces.
        assert!(m.get(m.sprite("t")).is_none());
        assert_eq!(m.state(m.sprite("t")), LoadState::Unknown);
    }

    #[test]
    fn deferred_loading_from_a_source() {
        let mut m =
            AssetManager::new(MemorySource::default().with("tex/a.png", png_bytes()).with("bad.png", vec![1, 2, 3]));
        let ok = m.load::<Texture>("tex/a.png").unwrap();
        let bad = m.load::<Texture>("bad.png").unwrap();
        let missing = m.load::<Texture>("nope.png").unwrap();
        assert_eq!(m.state(ok), LoadState::Loading);
        assert!(m.get(ok).is_none());
        assert!(m.has_pending_loads());

        assert_eq!(m.process_loads(), 3);
        assert_eq!(m.state(ok), LoadState::Loaded);
        let t = m.get(ok).unwrap();
        assert_eq!((t.width, t.height, t.rgba[4..8].to_vec()), (2, 1, vec![0, 255, 0, 255]));
        assert_eq!(m.state(bad), LoadState::Failed);
        assert!(m.error_of(bad).unwrap().contains("decode"));
        assert_eq!(m.state(missing), LoadState::Failed);

        // Loading again is a no-op for loaded assets.
        m.load::<Texture>("tex/a.png").unwrap();
        assert!(!m.has_pending_loads());
    }

    #[test]
    fn a_failed_load_warns_once_per_occurrence_and_names_the_asset() {
        capture::begin();
        let mut m = AssetManager::in_memory();
        let bad = m.load::<Texture>("nope.png").unwrap();
        assert_eq!(m.state(bad), LoadState::Loading);
        assert!(capture::messages().is_empty(), "a load that has not been processed is not a failure");

        assert_eq!(m.process_loads(), 1);
        assert_eq!(m.state(bad), LoadState::Failed);
        let first = capture::messages();
        assert_eq!(first.len(), 1, "{first:?}");
        assert!(first[0].contains("asset 'nope.png' failed to load"), "{first:?}");
        assert_eq!(capture::records()[0].level, log::Level::Warn);
        assert!(m.error_of(bad).is_some());

        assert_eq!(m.process_loads(), 0);
        assert_eq!(capture::messages().len(), 1, "processing again does not repeat the warning");

        m.load::<Texture>("nope.png").unwrap();
        m.process_loads();
        let again = capture::messages();
        assert_eq!(again.len(), 2, "{again:?}");
        assert!(again[1].contains("asset 'nope.png' failed to load"));
    }

    #[test]
    fn types_without_decoders_fail_cleanly() {
        let mut m = AssetManager::new(MemorySource::default().with("m", vec![0]));
        let h = m.load::<MeshData>("m").unwrap();
        m.process_loads();
        assert_eq!(m.state(h), LoadState::Failed);
    }

    #[test]
    fn dependencies_are_tracked() {
        let mut m = AssetManager::in_memory();
        let tex = m.insert("tex", Texture::solid(2, 2, [0; 4])).unwrap();
        let spr = m.insert("spr", SpriteAsset::whole_texture(tex, 2, 2)).unwrap();
        m.add_dependency(spr, tex);
        m.add_dependency(spr, tex);
        assert_eq!(m.dependencies(spr), &[tex.id()]);
    }

    #[test]
    fn remove_forgets_the_entry() {
        let mut m = AssetManager::in_memory();
        let h = m.insert("t", Texture::solid(1, 1, [1, 2, 3, 4])).unwrap();
        assert!(m.remove(h));
        assert!(m.get(h).is_none());
        assert_eq!(m.state(h), LoadState::Unknown);
        assert_eq!(m.revision(h), 0);
        assert!(m.name_of(h).is_none());
        assert!(m.error_of(h).is_none());
        assert!(!m.remove(h), "removing again reports that nothing was there");
        assert!(!m.remove(m.texture("never inserted")));
    }

    #[test]
    fn remove_also_forgets_a_failed_load() {
        let mut m = AssetManager::in_memory();
        let h = m.load::<Texture>("missing.png").unwrap();
        m.process_loads();
        assert_eq!(m.state(h), LoadState::Failed);
        assert!(m.remove(h));
        assert_eq!(m.state(h), LoadState::Unknown);
        assert!(m.error_of(h).is_none());
    }

    #[test]
    fn remove_drops_dependency_links_in_both_directions() {
        let mut m = AssetManager::in_memory();
        let tex = m.insert("tex", Texture::solid(2, 2, [0; 4])).unwrap();
        let other = m.insert("other", Texture::solid(2, 2, [0; 4])).unwrap();
        let spr = m.insert("spr", SpriteAsset::whole_texture(tex, 2, 2)).unwrap();
        m.add_dependency(spr, tex);
        m.add_dependency(spr, other);

        // Removing the child drops the parent's link to it and keeps the others.
        m.remove(tex);
        assert_eq!(m.dependencies(spr), &[other.id()]);

        // Removing the parent drops its own links with it.
        m.remove(spr);
        assert!(m.dependencies(spr).is_empty());
        let spr_again = m.insert("spr", SpriteAsset::whole_texture(other, 2, 2)).unwrap();
        assert!(m.dependencies(spr_again).is_empty(), "a re-inserted asset starts with no links");
    }

    #[test]
    fn remove_keeps_links_while_another_type_still_has_that_id() {
        let mut m = AssetManager::in_memory();
        let tex = m.insert("shared", Texture::solid(1, 1, [0; 4])).unwrap();
        let mesh = m.insert("shared", MeshData::cuboid(engine_math::Vec3::ONE).unwrap()).unwrap();
        let spr = m.insert("spr", SpriteAsset::whole_texture(tex, 1, 1)).unwrap();
        m.add_dependency(spr, tex);
        m.remove(tex);
        assert_eq!(m.dependencies(spr), &[tex.id()]);
        assert!(m.get(mesh).is_some());
        m.remove(mesh);
        assert!(m.dependencies(spr).is_empty());
    }

    #[test]
    fn remove_only_affects_its_own_type() {
        let mut m = AssetManager::in_memory();
        let tex = m.insert("same name", Texture::solid(1, 1, [0; 4])).unwrap();
        let mesh = m.insert("same name", MeshData::cuboid(engine_math::Vec3::ONE).unwrap()).unwrap();
        assert!(m.remove(tex));
        assert!(m.get(tex).is_none());
        assert!(m.get(mesh).is_some());
    }

    #[test]
    fn remove_cancels_a_pending_load() {
        let mut m = AssetManager::new(MemorySource::default().with("tex/a.png", png_bytes()));
        let h = m.load::<Texture>("tex/a.png").unwrap();
        let kept = m.load::<Texture>("tex/a.png").unwrap();
        assert_eq!(m.state(h), LoadState::Loading);
        assert!(m.remove(h));
        assert!(!m.has_pending_loads());
        assert_eq!(m.process_loads(), 0);
        assert_eq!(m.state(kept), LoadState::Unknown, "the cancelled load must not resurrect the asset");
    }

    #[test]
    fn remove_leaves_other_pending_loads_alone() {
        let mut m = AssetManager::new(MemorySource::default().with("a.png", png_bytes()).with("b.png", png_bytes()));
        let a = m.load::<Texture>("a.png").unwrap();
        let b = m.load::<Texture>("b.png").unwrap();
        m.remove(a);
        assert_eq!(m.process_loads(), 1);
        assert_eq!(m.state(a), LoadState::Unknown);
        assert_eq!(m.state(b), LoadState::Loaded);
    }

    #[test]
    fn a_reinserted_asset_never_reuses_an_old_revision() {
        let mut m = AssetManager::in_memory();
        let h = m.insert("t", Texture::solid(1, 1, [1, 1, 1, 1])).unwrap();
        let first = m.revision(h);
        m.remove(h);
        m.insert("t", Texture::solid(1, 1, [2, 2, 2, 2])).unwrap();
        let second = m.revision(h);
        assert_ne!(first, second, "a GPU cache keyed by (id, revision) would show the stale texture");
        assert_ne!(second, 0);
        m.insert("t", Texture::solid(1, 1, [3, 3, 3, 3])).unwrap();
        assert_ne!(m.revision(h), second);
    }
    // Two real names that collide would take a ~2^32-name search, so these tests choose the id.
    const CHOSEN: AssetId = AssetId(0x0C01_11DE);

    fn texture_with(rgba: [u8; 4]) -> Texture {
        Texture::solid(1, 1, rgba)
    }

    #[test]
    fn a_different_name_with_the_same_id_is_rejected_and_never_overwrites() {
        let mut m = AssetManager::in_memory();
        let first = m.insert_with_id(CHOSEN, "first", texture_with([1, 1, 1, 1])).unwrap();
        let revision = m.revision(first);

        let err = m.insert_with_id(CHOSEN, "second", texture_with([2, 2, 2, 2])).unwrap_err();
        match &err {
            AssetError::IdCollision { id, existing, new } => {
                assert_eq!((*id, existing.as_str(), new.as_str()), (CHOSEN, "first", "second"));
            }
            other => panic!("expected IdCollision, got {other:?}"),
        }
        assert!(err.to_string().contains("`first`") && err.to_string().contains("`second`"), "{err}");

        assert_eq!(m.get(first).unwrap().rgba, vec![1, 1, 1, 1], "the stored value is untouched");
        assert_eq!(m.name_of(first), Some("first"));
        assert_eq!(m.revision(first), revision, "no change, so no GPU re-upload");
        assert_eq!(m.state(first), LoadState::Loaded);
    }

    #[test]
    fn inserting_the_same_name_again_still_replaces() {
        let mut m = AssetManager::in_memory();
        let h = m.insert_with_id(CHOSEN, "same", texture_with([1, 1, 1, 1])).unwrap();
        let before = m.revision(h);
        m.insert_with_id(CHOSEN, "same", texture_with([2, 2, 2, 2])).unwrap();
        assert_eq!(m.get(h).unwrap().rgba, vec![2, 2, 2, 2]);
        assert_ne!(m.revision(h), before);
    }

    #[test]
    fn the_same_id_under_a_different_type_is_not_a_collision() {
        let mut m = AssetManager::in_memory();
        let tex = m.insert_with_id(CHOSEN, "one", texture_with([1, 1, 1, 1])).unwrap();
        let mesh = m.insert_with_id(CHOSEN, "two", MeshData::cuboid(engine_math::Vec3::ONE).unwrap()).unwrap();
        assert!(m.get(tex).is_some() && m.get(mesh).is_some());
    }

    #[test]
    fn a_removed_asset_frees_its_id_for_another_name() {
        let mut m = AssetManager::in_memory();
        let h = m.insert_with_id(CHOSEN, "old", texture_with([1, 1, 1, 1])).unwrap();
        assert!(m.remove(h));
        m.insert_with_id(CHOSEN, "new", texture_with([2, 2, 2, 2])).unwrap();
        assert_eq!(m.name_of(h), Some("new"));
    }

    #[test]
    fn load_of_a_colliding_name_is_rejected_and_requests_nothing() {
        let mut m = AssetManager::new(MemorySource::default().with("b.png", png_bytes()));
        let a = m.insert_with_id(CHOSEN, "a.png", texture_with([1, 1, 1, 1])).unwrap();
        let err = m.load_with_id::<Texture>(CHOSEN, "b.png").unwrap_err();
        assert!(matches!(err, AssetError::IdCollision { .. }), "{err:?}");
        assert!(!m.has_pending_loads());
        assert_eq!(m.process_loads(), 0);
        assert_eq!(m.get(a).unwrap().rgba, vec![1, 1, 1, 1]);
        assert_eq!(m.name_of(a), Some("a.png"));
    }

    #[test]
    fn load_of_a_name_that_collides_with_a_load_in_flight_is_rejected() {
        let mut m = AssetManager::new(MemorySource::default().with("a.png", png_bytes()).with("b.png", png_bytes()));
        let a = m.load_with_id::<Texture>(CHOSEN, "a.png").unwrap();
        assert!(m.load_with_id::<Texture>(CHOSEN, "b.png").is_err());
        assert_eq!(m.process_loads(), 1);
        assert_eq!(m.state(a), LoadState::Loaded);
        assert_eq!(m.name_of(a), Some("a.png"));
    }

    #[test]
    fn insert_of_a_name_that_collides_with_a_failed_load_is_rejected() {
        let mut m = AssetManager::in_memory();
        let a = m.load_with_id::<Texture>(CHOSEN, "missing.png").unwrap();
        m.process_loads();
        assert_eq!(m.state(a), LoadState::Failed);
        assert!(m.insert_with_id(CHOSEN, "other", texture_with([0; 4])).is_err());
        assert_eq!(m.state(a), LoadState::Failed, "the failed entry keeps its error");
    }

    #[test]
    fn distinct_names_do_not_collide() {
        let mut m = AssetManager::in_memory();
        for i in 0..2000 {
            m.insert(&format!("asset_{i}"), texture_with([0; 4])).unwrap();
        }
    }
}
