mod custom_storage;
mod delete_any;
mod retain;

pub use custom_storage::CustomStorageAccess;
pub use delete_any::{CustomDeleteAny, DeleteAny};
pub use retain::Retain;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::borrow::{AllStoragesBorrow, Borrow, IntoBorrow};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::error;
use crate::memory_usage::AllStoragesMemoryUsage;
use crate::reserve::BulkEntityIter;
use crate::sparse_set::{AddComponent, BulkAddEntity, DeleteComponent, Remove};
use crate::storage::{SBox, Storage, StorageId};
use crate::unique::Unique;
use alloc::boxed::Box;
use core::any::type_name;
use core::cell::UnsafeCell;
use hashbrown::hash_map::{Entry, HashMap};
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
    storages: UnsafeCell<HashMap<StorageId, SBox>>,
    #[cfg(feature = "thread_local")]
    thread_id: std::thread::ThreadId,
}

#[cfg(not(feature = "thread_local"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    pub(crate) fn new() -> Self {
        let mut storages = HashMap::new();

        storages.insert(StorageId::of::<Entities>(), SBox::new(Entities::new()));

        AllStorages {
            storages: UnsafeCell::new(storages),
            lock: RawRwLock::INIT,
            #[cfg(feature = "thread_local")]
            thread_id: std::thread::current().id(),
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(0usize);
    /// ```
    ///
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| SBox::new(Unique::new(component)));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSend]: crate::NonSend
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_send<T: 'static + Sync>(&self, component: T) {
        if std::thread::current().id() == self.thread_id {
            let storage_id = StorageId::of::<Unique<T>>();

            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send(Unique::new(component), self.thread_id));
            unsafe { self.lock.unlock_exclusive() };
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// [NonSync]: crate::NonSync
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_sync<T: 'static + Send>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages
            .entry(storage_id)
            .or_insert_with(|| SBox::new_non_sync(Unique::new(component)));
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [NonSync]: crate::NonSync
    /// [UniqueView]: crate::UniqueView
    /// [UniqueViewMut]: crate::UniqueViewMut
    #[cfg(feature = "thread_local")]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) {
        if std::thread::current().id() == self.thread_id {
            let storage_id = StorageId::of::<Unique<T>>();

            self.lock.lock_exclusive();
            let storages = unsafe { &mut *self.storages.get() };
            storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new_non_send_sync(Unique::new(component), self.thread_id));
            unsafe { self.lock.unlock_exclusive() };
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.add_unique(0usize);
    /// let i = all_storages.remove_unique::<usize>().unwrap();
    /// ```
    pub fn remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();

        {
            // SAFE we locked
            let storages = unsafe { &mut *self.storages.get() };

            let storage = if let Entry::Occupied(entry) = storages.entry(storage_id) {
                // `.err()` to avoid borrowing `entry` in the `Ok` case
                if let Some(err) = unsafe { &*entry.get().0 }.borrow_mut().err() {
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

            let unique: Box<AtomicRefCell<Unique<T>>> =
                unsafe { Box::from_raw(storage.0 as *mut AtomicRefCell<Unique<T>>) };

            core::mem::forget(storage);

            Ok(unique.into_inner().value)
        }
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
    /// }).unwrap();
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// let entity = all_storages.add_entity((0u32, 1usize));
    ///
    /// all_storages.strip(entity);
    /// ```
    pub fn strip(&mut self, entity: EntityId) {
        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().delete(entity);
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
        for (storage_id, storage) in self.storages.get_mut().iter_mut() {
            if !excluded_storage.contains(&*storage_id) {
                unsafe { &mut *storage.0 }.get_mut().delete(entity);
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// all_storages.clear();
    /// ```
    pub fn clear(&mut self) {
        for storage in self.storages.get_mut().values_mut() {
            unsafe { &mut *storage.0 }.get_mut().clear();
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
    /// ### Panics
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{AllStoragesViewMut, World};
    ///
    /// let mut world = World::new();
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    ///
    /// // make an empty entity
    /// let entity = all_storages.add_entity(());
    ///
    /// all_storages.add_component(entity, (0u32,));
    /// // entity already had a `u32` component so it will be replaced
    /// all_storages.add_component(entity, (1u32, 11usize));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_component<T: AddComponent>(&mut self, entity: EntityId, component: T) {
        if self
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(entity)
        {
            component.add_component(self, entity);
        } else {
            panic!("{:?}", error::AddComponent::EntityIsNotAlive);
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
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

let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let u32s = all_storages.borrow::<View<u32>>().unwrap();
let (entities, mut usizes) = all_storages
    .borrow::<(EntitiesView, ViewMut<usize>)>()
    .unwrap();
```
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    pub fn borrow<'s, V: IntoBorrow>(&'s self) -> Result<V, error::GetStorage>
    where
        V::Borrow: Borrow<'s, View = V> + AllStoragesBorrow<'s>,
    {
        V::Borrow::all_borrow(self)
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
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- Storage (exclusive or shared)
### Errors

- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    pub fn run_with_data<'s, Data, B, R, S: crate::system::AllSystem<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> Result<R, error::Run> {
        s.run((data,), self).map_err(error::Run::GetStorage)
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
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSend]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send`  
[NonSend] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[NonSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSync: must activate the *thread_local* feature"
    )]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "    * [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "thread_local", not(docsrs)),
        doc = "* [NonSendSync]<[View]\\<T\\>> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<[ViewMut]\\<T\\>> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[NonSendSync] and [UniqueView]/[UniqueViewMut] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSendSync: must activate the *thread_local* feature"
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

let all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

all_storages
    .run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
        // -- snip --
    })
    .unwrap();

let i = all_storages.run(sys1).unwrap();
```
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    pub fn run<'s, B, R, S: crate::system::AllSystem<'s, (), B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        s.run((), self).map_err(error::Run::GetStorage)
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
    /// let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
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
        let storage = unsafe { &*storage.0 }.borrow();
        unsafe { self.lock.unlock_shared() };
        match storage {
            Ok(storage) => Ok(Ref::map(storage, |storage| {
                storage.as_any().downcast_ref().unwrap()
            })),
            Err(err) => Err(error::GetStorage::Entities(err)),
        }
    }
    pub(crate) fn entities_mut(&self) -> Result<RefMut<'_, &'_ mut Entities>, error::GetStorage> {
        let storage_id = StorageId::of::<Entities>();

        self.lock.lock_shared();
        let storages = unsafe { &*self.storages.get() };
        let storage = storages.get(&storage_id).unwrap();
        let storage = unsafe { &*storage.0 }.borrow_mut();
        unsafe { self.lock.unlock_shared() };
        match storage {
            Ok(storage) => Ok(RefMut::map(storage, |storage| {
                storage.as_any_mut().downcast_mut().unwrap()
            })),
            Err(err) => Err(error::GetStorage::Entities(err)),
        }
    }
    pub(crate) fn exclusive_storage_mut<T: 'static>(
        &mut self,
    ) -> Result<&mut T, error::GetStorage> {
        self.exclusive_storage_mut_by_id(StorageId::of::<T>())
    }
    pub(crate) fn exclusive_storage_mut_by_id<T: 'static>(
        &mut self,
        storage_id: StorageId,
    ) -> Result<&mut T, error::GetStorage> {
        let storages = unsafe { &mut *self.storages.get() };
        if let Some(storage) = storages.get_mut(&storage_id) {
            let storage = unsafe { &mut *storage.0 }
                .get_mut()
                .as_any_mut()
                .downcast_mut()
                .unwrap();
            Ok(storage)
        } else {
            Err(error::GetStorage::MissingStorage {
                name: Some(type_name::<T>()),
                id: StorageId::of::<T>(),
            })
        }
    }
    pub(crate) fn exclusive_storage_or_insert_mut<T, F>(
        &mut self,
        storage_id: StorageId,
        f: F,
    ) -> &mut T
    where
        T: 'static + Storage + Send + Sync,
        F: FnOnce() -> T,
    {
        let storages = unsafe { &mut *self.storages.get() };

        unsafe {
            &mut *storages
                .entry(storage_id)
                .or_insert_with(|| SBox::new(f()))
                .0
        }
        .get_mut()
        .as_any_mut()
        .downcast_mut()
        .unwrap()
    }
    /// Make the given entity alive.  
    /// Does nothing if an entity with a greater generation is already at this index.  
    /// Returns `true` if the entity is successfully spawned.
    #[inline]
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        self.exclusive_storage_mut::<Entities>()
            .unwrap()
            .spawn(entity)
    }
    /// Displays storages memory information.
    pub fn memory_usage(&self) -> AllStoragesMemoryUsage<'_> {
        AllStoragesMemoryUsage(self)
    }
}

impl core::fmt::Debug for AllStorages {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("AllStorages");

        self.lock.lock_shared();

        {
            let storages = unsafe { &*self.storages.get() };

            debug_struct.field("storage_count", &storages.len());
            debug_struct.field("storages", &storages.values());
        }

        unsafe { self.lock.unlock_shared() };

        debug_struct.finish()
    }
}

impl core::fmt::Debug for AllStoragesMemoryUsage<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut borrowed_storages = 0;

        let mut debug_struct = f.debug_list();

        self.0.lock.lock_shared();

        {
            let storages = unsafe { &*self.0.storages.get() };

            debug_struct.entries(storages.values().filter_map(|storage| {
                match unsafe { &*(storage.0) }.borrow() {
                    Ok(storage) => storage.memory_usage(),
                    Err(_) => {
                        borrowed_storages += 1;
                        None
                    }
                }
            }));
        }

        unsafe { self.0.lock.unlock_shared() };

        if borrowed_storages != 0 {
            debug_struct.entry(&format_args!(
                "{} storages could not be borrored",
                borrowed_storages
            ));
        }

        debug_struct.finish()
    }
}
