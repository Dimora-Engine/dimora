use std::any::{type_name, Any, TypeId};
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

/// Singleton values keyed by type (time, input, asset manager, ambient light...).
///
/// Borrows are checked at runtime like `RefCell`, which lets systems hold several resources and
/// query components at the same time through a shared `&World`.
#[derive(Default)]
pub struct Resources {
    map: HashMap<TypeId, RefCell<Box<dyn Any>>>,
}

impl Resources {
    pub fn insert<T: 'static>(&mut self, value: T) -> Option<T> {
        self.map
            .insert(TypeId::of::<T>(), RefCell::new(Box::new(value)))
            .and_then(|old| old.into_inner().downcast().ok().map(|b| *b))
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn get<T: 'static>(&self) -> Option<Ref<'_, T>> {
        let cell = self.map.get(&TypeId::of::<T>())?;
        Some(Ref::map(cell.borrow(), |b| b.downcast_ref::<T>().expect("resource type matches its TypeId")))
    }

    pub fn get_mut<T: 'static>(&self) -> Option<RefMut<'_, T>> {
        let cell = self.map.get(&TypeId::of::<T>())?;
        Some(RefMut::map(cell.borrow_mut(), |b| b.downcast_mut::<T>().expect("resource type matches its TypeId")))
    }

    /// `Ok(None)` when `T` was never inserted. `Err(())` when it is already borrowed.
    pub(crate) fn try_get_mut<T: 'static>(&self) -> Result<Option<RefMut<'_, T>>, ()> {
        let Some(cell) = self.map.get(&TypeId::of::<T>()) else {
            return Ok(None);
        };
        let guard = cell.try_borrow_mut().map_err(|_| ())?;
        Ok(Some(RefMut::map(guard, |b| b.downcast_mut::<T>().expect("resource type matches its TypeId"))))
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.map.remove(&TypeId::of::<T>()).and_then(|c| c.into_inner().downcast().ok().map(|b| *b))
    }
}

pub(crate) fn missing<T>() -> ! {
    panic!("resource `{}` has not been inserted into the World", type_name::<T>())
}
