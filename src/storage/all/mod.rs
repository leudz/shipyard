mod delete_any;
mod retain;

pub use delete_any::{CustomDeleteAny, DeleteAny};
pub use retain::Retain;

use super::{Entities, EntityId, Storage, StorageId, Unique};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::borrow::AllStoragesBorrow;
use crate::error;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::{AddComponent, BulkAddEntity, DeleteComponent, Remove};
use crate::unknown_storage::UnknownStorage;
use core::any::type_name;
use core::cell::UnsafeCell;
use indexmap::{map::Entry, IndexMap};
use parking_lot::{lock_api::RawRwLock as _, RawRwLock};

/// Contains all storages present in the `World`.
// The lock is held very briefly:
// - shared: when trying to find a storage
// - unique: when adding a storage
// once the storage is found or created the lock is released
// this is safe since World is still borrowed and there is no way to delete a storage
// so any access to storages are valid as long as the World exists
// we use a HashMap, it can reallocate, but even in this case the storages won't move since they are boxed
pub struct AllStorages {
    lock: RawRwLock,
    storages: UnsafeCell<IndexMap<StorageId, Storage>>,
    #[cfg(feature = "non_send")]
    thread_id: std::thread::ThreadId,
    inside_callback: UnsafeCell<bool>,
}

#[cfg(not(feature = "non_send"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    pub(crate) fn new() -> Self {
        let mut storages = IndexMap::new();

        storages.insert(StorageId::of::<Entities>(), Storage::new(Entities::new()));

        AllStorages {
            storages: UnsafeCell::new(storages),
            lock: RawRwLock::INIT,
            #[cfg(feature = "non_send")]
            thread_id: std::thread::current().id(),
            inside_callback: UnsafeCell::new(false),
        }
    }
    /// Removes a unique storage.  
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(0usize);
    /// let i = all_storages.try_remove_unique::<usize>().unwrap();
    /// ```
    pub fn try_remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();

        {
            if unsafe { *self.inside_callback.get() } {
                unsafe { self.lock.unlock_exclusive() };
                return Err(error::UniqueRemove::InsideCallback(type_name::<T>()));
            }

            // SAFE we locked
            let storages = unsafe { &mut *self.storages.get() };

            let storage = if let Entry::Occupied(entry) = storages.entry(storage_id) {
                // `.err()` to avoid borrowing `entry` in the `Ok` case
                if let Some(err) = entry.get().get_mut::<Unique<T>>().err() {
                    unsafe { self.lock.unlock_exclusive() };
                    return Err(error::UniqueRemove::StorageBorrow((type_name::<T>(), err)));
                } else {
                    // We were able to lock the storage, we've still got exclusive access even though
                    // we released that lock as we're still holding the `AllStorages` lock.
                    let storage = entry.remove();
                    unsafe { self.lock.unlock_exclusive() };
                    storage
                }
            } else {
                unsafe { self.lock.unlock_exclusive() };
                return Err(error::UniqueRemove::MissingUnique(type_name::<T>()));
            };

            let unique_ptr: *mut AtomicRefCell<Unique<T>> = storage.0 as _;

            core::mem::forget(storage);

            unsafe {
                let unique = core::ptr::read(unique_ptr);
                alloc::alloc::dealloc(
                    unique_ptr as *mut u8,
                    alloc::alloc::Layout::new::<AtomicRefCell<Unique<T>>>(),
                );

                Ok(unique.into_inner().value)
            }
        }
    }
    /// Removes a unique storage.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// all_storages.add_unique(0usize);
    /// let i = all_storages.remove_unique::<usize>();
    /// ```
    #[track_caller]
    pub fn remove_unique<T: 'static>(&self) -> T {
        match self.try_remove_unique::<T>() {
            Ok(unique) => unique,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [`UniqueView`] or [`UniqueViewMut`].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// all_storages.add_unique(0usize);
    /// ```
    ///
    /// [`UniqueView`]: struct.UniqueView.html
    /// [`UniqueViewMut`]: struct.UniqueViewMut.html
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| Storage::new(Unique::new(component)));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSend]: struct.NonSend.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_send")]
    pub fn add_unique_non_send<T: 'static + Sync>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| Storage::new_non_send(Unique::new(component), self.thread_id));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_sync")]
    pub fn add_unique_non_sync<T: 'static + Send>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| Storage::new_non_sync(Unique::new(component)));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| Storage::new_non_send_sync(Unique::new(component), self.thread_id));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, Get, View, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity1 = all_storages.add_entity((0usize, 1u32));
    /// let entity2 = all_storages.add_entity((2usize, 3u32));
    ///
    /// all_storages.delete_entity(entity1);
    ///
    /// all_storages.run(|usizes: View<usize>, u32s: View<u32>| {
    ///     assert!((&usizes).get(entity1).is_err());
    ///     assert!((&u32s).get(entity1).is_err());
    ///     assert_eq!(usizes.get(entity2), Ok(&2));
    ///     assert_eq!(u32s.get(entity2), Ok(&3));
    /// });
    /// ```
    pub fn delete_entity(&mut self, entity: EntityId) -> bool {
        // no need to lock here since we have a unique access
        let mut entities = self.entities_mut().unwrap();

        if entities.delete_unchecked(entity) {
            drop(entities);

            self.strip(entity);

            true
        } else {
            false
        }
    }
    /// Deletes all components from an entity without deleting it.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity = all_storages.add_entity((0u32, 1usize));
    ///
    /// all_storages.strip(entity);
    /// ```
    pub fn strip(&mut self, entity: EntityId) {
        let mut i = 0;
        let mut has_event = false;

        loop {
            {
                let storages = unsafe { &mut *self.storages.get() };

                while i < storages.len() {
                    let storage =
                        unsafe { (&mut *(storages.get_index_mut(i).unwrap().1).0).get_mut() };

                    storage.delete(entity);

                    if storage.has_remove_event_to_dispatch() {
                        has_event = true;
                        break;
                    }

                    i += 1;
                }
            }

            if has_event {
                has_event = false;
                let storages = unsafe { &*self.storages.get() };

                unsafe { *self.inside_callback.get() = true };
                let mut storage = unsafe { &*(storages.get_index(i).unwrap().1).0 }
                    .try_borrow_mut()
                    .unwrap();
                storage.run_on_remove_global(self);

                i += 1;
            } else {
                return;
            }
        }
    }
    /// Deletes all components of an entity except the ones passed in `S`.  
    /// The storage's type has to be used and not the component.  
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, SparseSet, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity = all_storages.add_entity((0u32, 1usize));
    ///
    /// all_storages.retain::<SparseSet<u32>>(entity);
    /// ```
    pub fn retain<S: Retain>(&mut self, entity: EntityId) {
        S::retain(self, entity);
    }
    /// Deletes all components of an entity except the ones passed in `S`.  
    /// This is identical to `retain` but uses `StorageId` and not generics.  
    /// You should only use this method if you use a custom storage with a runtime id.
    pub fn retain_storage(&mut self, entity: EntityId, excluded_storage: &[StorageId]) {
        let mut i = 0;
        let mut has_event = false;

        loop {
            {
                let storages = unsafe { &mut *self.storages.get() };

                while i < storages.len() {
                    let (storage_id, storage) = storages.get_index_mut(i).unwrap();

                    if !excluded_storage.contains(&*storage_id) {
                        let storage = unsafe { (&mut *storage.0).get_mut() };

                        storage.delete(entity);

                        if storage.has_remove_event_to_dispatch() {
                            has_event = true;
                            break;
                        }
                    }

                    i += 1;
                }
            }

            if has_event {
                has_event = false;
                let storages = unsafe { &*self.storages.get() };

                unsafe { *self.inside_callback.get() = true };
                let mut storage = unsafe { &*(storages.get_index(i).unwrap().1).0 }
                    .try_borrow_mut()
                    .unwrap();
                storage.run_on_remove_global(self);

                i += 1;
            } else {
                return;
            }
        }
    }
    /// Deletes all entities and components in the `World`.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// all_storages.clear();
    /// ```
    pub fn clear(&mut self) {
        let mut i = 0;
        let mut has_event = false;

        loop {
            {
                let storages = unsafe { &mut *self.storages.get() };

                while i < storages.len() {
                    let storage =
                        unsafe { (&mut *(storages.get_index_mut(i).unwrap().1).0).get_mut() };

                    storage.clear();

                    if storage.has_remove_event_to_dispatch() {
                        has_event = true;
                        break;
                    }

                    i += 1;
                }
            }

            if has_event {
                has_event = false;
                let storages = unsafe { &*self.storages.get() };

                unsafe { *self.inside_callback.get() = true };
                let mut storage = unsafe { &*(storages.get_index(i).unwrap().1).0 }
                    .try_borrow_mut()
                    .unwrap();
                storage.run_on_remove_global(self);

                i += 1;
            } else {
                return;
            }
        }
    }
    /// Creates a new entity with the components passed as argument and returns its `EntityId`.  
    /// `component` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity0 = all_storages.add_entity((0u32,));
    /// let entity1 = all_storages.add_entity((1u32, 11usize));
    /// ```
    #[inline]
    pub fn add_entity<T: AddComponent>(&mut self, component: T) -> EntityId {
        let entity = self.exclusive_storage_mut::<Entities>().unwrap().generate();
        component.add_component(self, entity);

        entity
    }
    /// Creates multiple new entities and returns an iterator yielding the new `EntityId`s.  
    /// `source` must always yield a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity0 = all_storages.bulk_add_entity((0..1).map(|_| {})).next();
    /// let entity1 = all_storages.bulk_add_entity((1..2).map(|i| (i as u32,))).next();
    /// let new_entities = all_storages.bulk_add_entity((10..20).map(|i| (i as u32, i)));
    /// ```
    #[inline]
    pub fn bulk_add_entity<T: BulkAddEntity>(&mut self, source: T) -> BulkEntityIter<'_> {
        source.bulk_add_entity(self)
    }
    /// Adds components to an existing entity.  
    /// If the entity already owned a component it will be replaced.  
    /// `component` must always be a tuple, even for a single component.
    ///
    /// ### Errors
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// // make an empty entity
    /// let entity = all_storages.add_entity(());
    ///
    /// all_storages.add_component(entity, (0u32,)).unwrap();
    /// // entity already had a `u32` component so it will be replaced
    /// all_storages.add_component(entity, (1u32, 11usize)).unwrap();
    /// ```
    #[inline]
    pub fn add_component<T: AddComponent>(
        &mut self,
        entity: EntityId,
        component: T,
    ) -> Result<(), error::AddComponent> {
        if self
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(entity)
        {
            component.add_component(self, entity);

            Ok(())
        } else {
            Err(error::AddComponent::EntityIsNotAlive)
        }
    }
    /// Removes components from an entity.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity = all_storages.add_entity((0u32, 1usize));
    ///
    /// let (i,) = all_storages.remove::<(u32,)>(entity);
    /// assert_eq!(i, Some(0));
    /// ```
    #[inline]
    pub fn remove<T: Remove>(&mut self, entity: EntityId) -> T::Out {
        T::remove(self, entity)
    }
    /// Deletes components from an entity. As opposed to `remove`, `delete` doesn't return anything.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity = all_storages.add_entity((0u32, 1usize));
    ///
    /// all_storages.delete_component::<(u32,)>(entity);
    /// ```
    #[inline]
    pub fn delete_component<C: DeleteComponent>(&mut self, entity: EntityId) {
        C::delete_component(self, entity);
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{AllStoragesViewMut, EntitiesView, View, ViewMut, World};

let world = World::new();

world.run(|all_storages: AllStoragesViewMut| {
    let u32s = all_storages.try_borrow::<View<u32>>().unwrap();
    let (entities, mut usizes) = all_storages
        .try_borrow::<(EntitiesView, ViewMut<usize>)>()
        .unwrap();
});
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_borrow<'s, V: AllStoragesBorrow<'s>>(&'s self) -> Result<V, error::GetStorage> {
        V::try_borrow(self)
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{AllStoragesViewMut, EntitiesView, View, ViewMut, World};

let world = World::new();

world.run(|all_storages: AllStoragesViewMut| {
    let u32s = all_storages.try_borrow::<View<u32>>().unwrap();
    let (entities, mut usizes) = all_storages.borrow::<(EntitiesView, ViewMut<usize>)>();
});
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn borrow<'s, V: AllStoragesBorrow<'s>>(&'s self) -> V {
        match self.try_borrow::<V>() {
            Ok(views) => views,
            Err(err) => panic!("{:?}", err),
        }
    }
    #[doc = "Borrows the requested storages and runs the function.  
Data can be passed to the function, this always has to be a single type but you can use a tuple if needed.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[World]: struct.World.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_run_with_data<'s, Data, B, R, S: crate::system::AllSystem<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> Result<R, error::Run> {
        Ok(s.run((data,), S::try_borrow(self)?))
    }
    #[doc = "Borrows the requested storages and runs the function.  
Data can be passed to the function, this always has to be a single type but you can use a tuple if needed.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[World]: struct.World.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run_with_data<'s, Data, B, R, S: crate::system::AllSystem<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> R {
        match self.try_run_with_data(s, data) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    #[doc = "Borrows the requested storages and runs the function.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{AllStoragesViewMut, View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>();

all_storages
    .try_run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
        // -- snip --
    })
    .unwrap();

let i = all_storages.try_run(sys1).unwrap();
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_run<'s, B, R, S: crate::system::AllSystem<'s, (), B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        Ok(s.run((), S::try_borrow(self)?))
    }
    #[doc = "Borrows the requested storages and runs the function.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [UniqueView]\\<T\\> for a shared access to a `T` unique storage
* [UniqueViewMut]\\<T\\> for an exclusive access to a `T` unique storage
* `Option<V>` with one or multiple views for fallible access to one or more storages"]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_send"),
        doc = "* NonSend: must activate the *non_send* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "non_sync"),
        doc = "* NonSync: must activate the *non_sync* feature"
    )]
    #[cfg_attr(
        all(feature = "non_sync", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code> and <code style=\"background-color: #C4ECFF\">feature=\"non_sync\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)

### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{AllStoragesViewMut, View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

let all_storages = world.borrow::<AllStoragesViewMut>();

all_storages.run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
    // -- snip --
});

let i = all_storages.run(sys1);
```
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[View]: struct.View.html
[ViewMut]: struct.ViewMut.html
[UniqueView]: struct.UniqueView.html
[UniqueViewMut]: struct.UniqueViewMut.html"]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run<'s, B, R, S: crate::system::AllSystem<'s, (), B, R>>(&'s self, s: S) -> R {
        match self.try_run(s) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Deletes any entity with at least one of the given type(s).  
    /// The storage's type has to be used and not the component.  
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, SparseSet, World};
    ///
    /// let world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>();
    ///
    /// let entity0 = all_storages.add_entity((0u32,));
    /// let entity1 = all_storages.add_entity((1usize,));
    /// let entity2 = all_storages.add_entity(("2",));
    ///
    /// // deletes `entity2`
    /// all_storages.delete_any::<SparseSet<&str>>();
    /// // deletes `entity0` and `entity1`
    /// all_storages.delete_any::<(SparseSet<u32>, SparseSet<usize>)>();
    /// ```
    pub fn delete_any<T: DeleteAny>(&mut self) {
        T::delete_any(self);
    }
    pub(crate) fn entities(&self) -> Result<Ref<'_, &'_ Entities>, error::GetStorage> {
        let storage_id = StorageId::of::<Entities>();

        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id).unwrap();
        let storage = storage.get::<Entities>();
        unsafe { self.lock.unlock_shared() };
        storage.map_err(error::GetStorage::Entities)
    }
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, &'_ mut Entities>, error::GetStorage> {
        let storage_id = StorageId::of::<Entities>();

        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id).unwrap();
        let storage = storage.get_mut::<Entities>();
        unsafe { self.lock.unlock_shared() };
        storage.map_err(error::GetStorage::Entities)
    }
    pub fn custom_storage<T: 'static>(&self) -> Result<Ref<'_, &'_ T>, error::GetStorage> {
        self.custom_storage_by_id(StorageId::of::<T>())
    }
    pub fn custom_storage_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<T>()))
        }
    }
    pub fn custom_storage_mut<T: 'static>(
        &self,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage> {
        self.custom_storage_mut_by_id(StorageId::of::<T>())
    }
    pub fn custom_storage_mut_by_id<T: 'static>(
        &self,
        storage_id: StorageId,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage> {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            Err(error::GetStorage::MissingStorage(type_name::<T>()))
        }
    }
    pub fn custom_storage_or_insert<T, F>(&self, f: F) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_by_id(StorageId::of::<T>(), f)
    }
    pub fn custom_storage_or_insert_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "non_send")]
    pub fn custom_storage_or_insert_non_send<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "non_send")]
    pub fn custom_storage_or_insert_non_send_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "non_sync")]
    pub fn custom_storage_or_insert_non_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_sync_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "non_sync")]
    pub fn custom_storage_or_insert_non_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn custom_storage_or_insert_non_send_sync<T, F>(
        &self,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_sync_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn custom_storage_or_insert_non_send_sync_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<Ref<'_, &'_ T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    pub fn custom_storage_or_insert_mut<T, F>(
        &self,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_mut_by_id(StorageId::of::<T>(), f)
    }
    pub fn custom_storage_or_insert_mut_by_id<T, F>(
        &self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'_, &'_ mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "non_send")]
    pub fn custom_storage_or_insert_non_send_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "non_send")]
    pub fn custom_storage_or_insert_non_send_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Sync,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(feature = "non_sync")]
    pub fn custom_storage_or_insert_non_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_sync_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(feature = "non_sync")]
    pub fn custom_storage_or_insert_non_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage + Send,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_sync(f()))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn custom_storage_or_insert_non_send_sync_mut<'a, T, F>(
        &'a self,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.custom_storage_or_insert_non_send_sync_mut_by_id(StorageId::of::<T>(), f)
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    pub fn custom_storage_or_insert_non_send_sync_mut_by_id<'a, T, F>(
        &'a self,
        storage_id: StorageId,
        f: F,
    ) -> Result<RefMut<'a, &'a mut T>, error::GetStorage>
    where
        T: 'static + UnknownStorage,
        F: FnOnce() -> T,
    {
        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut::<T>();
            drop(storages);
            unsafe { self.lock.unlock_shared() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        } else {
            unsafe { self.lock.unlock_shared() };
            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            let storage = storages
                .entry(storage_id)
                .or_insert_with(|| Storage::new_non_send_sync(f(), self.thread_id))
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
    pub(crate) fn exclusive_storage_mut<T: 'static>(
        &mut self,
    ) -> Result<&mut T, error::GetStorage> {
        self.exclusive_storage_mut_by_id(StorageId::of::<T>())
    }
    pub fn exclusive_storage_mut_by_id<T: 'static>(
        &mut self,
        storage_id: StorageId,
    ) -> Result<&mut T, error::GetStorage> {
        let storages = unsafe { &mut *self.storages.get() };
        let storage = storages.get_mut(&storage_id);
        if let Some(storage) = storage {
            let storage = storage.get_mut_exclusive::<T>();
            Ok(storage)
        } else {
            Err(error::GetStorage::MissingStorage(type_name::<T>()))
        }
    }
    pub(crate) fn exclusive_storage_or_insert_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + UnknownStorage + Send + Sync,
        F: FnOnce() -> T,
    {
        let storages = unsafe { &mut *self.storages.get() };

        storages
            .entry(storage_id)
            .or_insert_with(|| Storage::new(f()))
            .get_mut_exclusive()
    }
}
