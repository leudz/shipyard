mod delete_any;

pub use delete_any::DeleteAny;

use super::{Entities, EntityId, Storage, StorageId, Unique};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::borrow::AllStoragesBorrow;
use crate::entity_builder::EntityBuilder;
use crate::error;
use crate::unknown_storage::UnknownStorage;
use alloc::boxed::Box;
use core::any::type_name;
use core::cell::UnsafeCell;
use hashbrown::{hash_map::Entry, HashMap};
use parking_lot::{lock_api::RawRwLock as _, RawRwLock};

/// Contains all components present in the World.
// The lock is held very briefly:
// - shared: when trying to find a storage
// - unique: when adding a storage
// once the storage is found or created the lock is released
// this is safe since World is still borrowed and there is no way to delete a storage
// so any access to storages are valid as long as the World exists
// we use a HashMap, it can reallocate, but even in this case the storages won't move since they are boxed
pub struct AllStorages {
    lock: RawRwLock,
    storages: UnsafeCell<HashMap<StorageId, Storage>>,
    #[cfg(feature = "non_send")]
    thread_id: std::thread::ThreadId,
}

#[cfg(not(feature = "non_send"))]
unsafe impl Send for AllStorages {}

unsafe impl Sync for AllStorages {}

impl AllStorages {
    pub(crate) fn new() -> Self {
        let mut storages = HashMap::default();

        let entities = Entities::new();

        storages.insert(
            StorageId::of::<Entities>(),
            Storage(Box::new(AtomicRefCell::new(entities))),
        );

        AllStorages {
            storages: UnsafeCell::new(storages),
            lock: RawRwLock::INIT,
            #[cfg(feature = "non_send")]
            thread_id: std::thread::current().id(),
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
    pub fn try_remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        let type_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        // SAFE we locked
        let storages = unsafe { &mut *self.storages.get() };
        if let Entry::Occupied(entry) = storages.entry(type_id) {
            // `.err()` to avoid borrowing `entry` in the `Ok` case
            if let Some(err) = entry.get().get_mut::<Unique<T>>().err() {
                unsafe { self.lock.unlock_exclusive() };
                Err(error::UniqueRemove::StorageBorrow((type_name::<T>(), err)))
            } else {
                // We were able to lock the storage, we've still got exclusive access even though
                // we released that lock as we're still holding the `AllStorages` lock.
                let storage = entry.remove();
                unsafe { self.lock.unlock_exclusive() };

                let ptr = Box::into_raw(storage.0);
                let unique_ptr: *mut AtomicRefCell<Unique<T>> = ptr as _;
                unsafe {
                    let unique = core::ptr::read(unique_ptr);
                    alloc::alloc::dealloc(
                        unique_ptr as *mut u8,
                        alloc::alloc::Layout::new::<AtomicRefCell<Unique<T>>>(),
                    );
                    Ok(unique.into_inner().value)
                }
            }
        } else {
            unsafe { self.lock.unlock_exclusive() };
            Err(error::UniqueRemove::MissingUnique(type_name::<T>()))
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
    #[track_caller]
    pub fn remove_unique<T: 'static>(&self) -> T {
        match self.try_remove_unique::<T>() {
            Ok(unique) => unique,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    ///
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        let storage_id = StorageId::of::<Unique<T>>();

        self.lock.lock_exclusive();
        let storages = unsafe { &mut *self.storages.get() };
        storages.entry(storage_id).or_insert_with(|| {
            Storage(Box::new(AtomicRefCell::new(Unique {
                value: component,
                is_modified: false,
            })))
        });
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
        storages.entry(storage_id).or_insert_with(|| {
            Storage(Box::new(AtomicRefCell::new_non_send(
                Unique {
                    value: component,
                    is_modified: false,
                },
                self.thread_id,
            )))
        });
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
        storages.entry(storage_id).or_insert_with(|| {
            Storage(Box::new(AtomicRefCell::new_non_sync(Unique {
                value: component,
                is_modified: false,
            })))
        });
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
        storages.entry(storage_id).or_insert_with(|| {
            Storage(Box::new(AtomicRefCell::new_non_send_sync(
                Unique {
                    value: component,
                    is_modified: false,
                },
                self.thread_id,
            )))
        });
        unsafe { self.lock.unlock_exclusive() };
    }
    /// Delete an entity and all its components.
    /// Returns `true` if `entity` was alive.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{AllStoragesViewMut, EntitiesViewMut, Get, View, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// let [entity1, entity2] = world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         [
    ///             entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
    ///             entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
    ///         ]
    ///     },
    /// );
    ///
    /// world.run(|mut all_storages: AllStoragesViewMut| {
    ///     all_storages.delete(entity1);
    /// });
    ///
    /// world.run(|usizes: View<usize>, u32s: View<u32>| {
    ///     assert!((&usizes).get(entity1).is_err());
    ///     assert!((&u32s).get(entity1).is_err());
    ///     assert_eq!(usizes.get(entity2), Ok(&2));
    ///     assert_eq!(u32s.get(entity2), Ok(&3));
    /// });
    /// ```
    pub fn delete(&mut self, entity: EntityId) -> bool {
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
    pub fn strip(&mut self, entity: EntityId) {
        // SAFE we have unique access
        let storages = unsafe { &mut *self.storages.get() };

        for storage in storages.values_mut() {
            // we have unique access to all storages so we can unwrap
            storage.delete(entity).unwrap();
        }
    }
    /// Deletes all entities and their components.
    pub fn clear(&mut self) {
        // SAFE we have unique access
        let storages = unsafe { &mut *self.storages.get() };

        for storage in storages.values_mut() {
            // we have unique access to all storages so we can unwrap
            storage.clear().unwrap()
        }
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
    ///
    /// `T` has to be a tuple even for a single type.  
    /// In this case use (T,).
    pub fn delete_any<T: DeleteAny>(&mut self) {
        T::delete_any(self)
    }
    /// Used to create an entity without having to borrow its storage explicitly.  
    /// The entity is only added when [EntityBuilder::try_build] or [EntityBuilder::build] is called.
    ///
    /// [EntityBuilder::try_build]: struct.EntityBuilder.html#method.try_build
    /// [EntityBuilder::build]: struct.EntityBuilder.html#method.build
    /// [AllStorages]: struct.AllStorages.html
    pub fn entity_builder(&self) -> EntityBuilder<'_, (), ()> {
        EntityBuilder::new_from_reference(self)
    }
    // #[cfg(feature = "serde1")]
    // pub(crate) fn storages(&mut self) -> &mut HashMap<StorageId, Storage> {
    //     // SAFE we have exclusive access
    //     unsafe { &mut *self.storages.get() }
    // }
    /// Shares all `owned`'s components with `shared` entity.  
    /// Deleting `owned`'s component won't stop the sharing.  
    /// Trying to share an entity with itself won't do anything.
    pub fn share(&mut self, owned: EntityId, shared: EntityId) {
        // SAFE we have unique access
        let storages = unsafe { &mut *self.storages.get() };

        for storage in storages.values_mut() {
            // we have unique access to all storages so we can unwrap
            storage.share(owned, shared).unwrap()
        }
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
                .or_insert_with(|| Storage(Box::new(AtomicRefCell::new(f()))))
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
                .or_insert_with(|| {
                    Storage(Box::new(AtomicRefCell::new_non_send(f(), self.thread_id)))
                })
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
                .or_insert_with(|| Storage(Box::new(AtomicRefCell::new_non_sync(f()))))
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
                .or_insert_with(|| {
                    Storage(Box::new(AtomicRefCell::new_non_send_sync(
                        f(),
                        self.thread_id,
                    )))
                })
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
                .or_insert_with(|| Storage(Box::new(AtomicRefCell::new(f()))))
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
                .or_insert_with(|| {
                    Storage(Box::new(AtomicRefCell::new_non_send(f(), self.thread_id)))
                })
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
                .or_insert_with(|| Storage(Box::new(AtomicRefCell::new_non_sync(f()))))
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
                .or_insert_with(|| {
                    Storage(Box::new(AtomicRefCell::new_non_send_sync(
                        f(),
                        self.thread_id,
                    )))
                })
                .get_mut();
            unsafe { self.lock.unlock_exclusive() };
            storage.map_err(|err| error::GetStorage::StorageBorrow((type_name::<T>(), err)))
        }
    }
}

// #[cfg(feature = "serde1")]
// pub(crate) struct AllStoragesSerializer<'a> {
//     pub(crate) all_storages: RefMut<'a, AllStorages>,
//     pub(crate) ser_config: crate::serde_setup::GlobalSerConfig,
// }

// #[cfg(feature = "serde1")]
// impl serde::Serialize for AllStoragesSerializer<'_> {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         use serde::ser::SerializeStruct;

//         let all_storages = unsafe { &*self.all_storages.storages.get() };

//         let mut storages = Vec::with_capacity(all_storages.len());

//         let ser_infos = crate::serde_setup::SerInfos {
//             same_binary: self.ser_config.same_binary,
//             with_entities: self.ser_config.with_entities,
//         };

//         let mut state = serializer.serialize_struct("AllStorages", 3)?;

//         if ser_infos.same_binary {
//             let metadata = all_storages
//                 .iter()
//                 .filter_map(|(type_id, storage)| {
//                     let storage = storage.0.try_borrow().unwrap();

//                     if storage.should_serialize(self.ser_config) {
//                         let result = match storage.deserialize() {
//                             Some(deserialize) => Ok((
//                                 type_id,
//                                 crate::unknown_storage::deserialize_ptr(deserialize),
//                             )),
//                             None => Err(serde::ser::Error::custom(
//                                 "Unknown storage's implementation is incorrect.",
//                             )),
//                         };

//                         storages.push(storage);

//                         Some(result)
//                     } else {
//                         None
//                     }
//                 })
//                 .collect::<Result<Vec<_>, S::Error>>()?;

//             state.serialize_field("ser_infos", &ser_infos)?;
//             state.serialize_field("metadata", &metadata)?;
//             state.serialize_field(
//                 "storages",
//                 &storages
//                     .iter()
//                     .map(|storage| crate::unknown_storage::StorageSerializer {
//                         unknown_storage: &**storage,
//                         ser_config: self.ser_config,
//                     })
//                     .collect::<Vec<_>>(),
//             )?;
//         } else {
//             todo!()
//         }

//         state.end()
//     }
// }
