pub mod scheduler;

pub use scheduler::{Workload, WorkloadBuilder};

pub(crate) use scheduler::TypeInfo;

use crate::atomic_refcell::AtomicRefCell;
// #[cfg(feature = "serde1")]
// use crate::atomic_refcell::RefMut;
use crate::borrow::Borrow;
use crate::entity_builder::EntityBuilder;
use crate::error;
// #[cfg(feature = "serde1")]
// use crate::serde_setup::{ExistingEntities, GlobalDeConfig, GlobalSerConfig, WithShared};
use crate::storage::AllStorages;
// #[cfg(feature = "serde1")]
// use crate::storage::{Storage, StorageId};
use alloc::borrow::Cow;
use alloc::vec::Vec;
use scheduler::Scheduler;

/// Holds all components and keeps track of entities and what they own.
pub struct World {
    pub(crate) all_storages: AtomicRefCell<AllStorages>,
    scheduler: AtomicRefCell<Scheduler>,
}

impl Default for World {
    /// Create an empty `World`.
    fn default() -> Self {
        World {
            #[cfg(not(feature = "non_send"))]
            all_storages: AtomicRefCell::new(AllStorages::new()),
            #[cfg(feature = "non_send")]
            all_storages: AtomicRefCell::new_non_send(
                AllStorages::new(),
                std::thread::current().id(),
            ),
            scheduler: AtomicRefCell::new(Default::default()),
        }
    }
}

impl World {
    /// Create an empty `World`.
    pub fn new() -> Self {
        Default::default()
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        match self.try_add_unique(component) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [UniqueView] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    pub fn try_add_unique<T: 'static + Send + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages.try_borrow()?.add_unique(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSend]: struct.NonSend.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    pub fn try_add_unique_non_send<T: 'static + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_send(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSend] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSend]: struct.NonSend.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_send", feature = "panic"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "panic"))))]
    #[track_caller]
    pub fn add_unique_non_send<T: 'static + Sync>(&self, component: T) {
        match self.try_add_unique_non_send::<T>(component) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    pub fn try_add_unique_non_sync<T: 'static + Send>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_sync(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSync]: struct.NonSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_sync", feature = "panic"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_sync", feature = "panic"))))]
    #[track_caller]
    pub fn add_unique_non_sync<T: 'static + Send>(&self, component: T) {
        match self.try_add_unique_non_sync::<T>(component) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSendSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSendSync]: struct.NonSendSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    pub fn try_add_unique_non_send_sync<T: 'static>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_send_sync(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T`.  
    /// To access a unique storage value, use [NonSendSync] and [UniqueViewMut] or [UniqueViewMut].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [NonSendSync]: struct.NonSendSync.html
    /// [UniqueView]: struct.UniqueView.html
    /// [UniqueViewMut]: struct.UniqueViewMut.html
    #[cfg(all(feature = "non_send", feature = "non_sync", feature = "panic"))]
    #[cfg_attr(
        docsrs,
        doc(cfg(all(feature = "non_send", feature = "non_sync", feature = "panic")))
    )]
    #[track_caller]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) {
        match self.try_add_unique_non_send_sync::<T>(component) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Removes a unique storage.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    ///
    /// [AllStorages]: struct.AllStorages.html
    pub fn try_remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        self.all_storages
            .try_borrow()
            .map_err(|_| error::UniqueRemove::AllStorages)?
            .try_remove_unique::<T>()
    }
    /// Removes a unique storage.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    /// - `T` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    /// - `T` storage borrow failed.
    /// - `T` storage did not exist.
    ///
    /// [AllStorages]: struct.AllStorages.html
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn remove_unique<T: 'static>(&self) -> T {
        match self.try_remove_unique() {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{EntitiesView, View, ViewMut, World};

let world = World::new();

let u32s = world.try_borrow::<View<u32>>().unwrap();
let (entities, mut usizes) = world
    .try_borrow::<(EntitiesView, ViewMut<usize>)>()
    .unwrap();
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn try_borrow<'s, V: Borrow<'s>>(&'s self) -> Result<V, error::GetStorage> {
        #[cfg(feature = "parallel")]
        {
            V::try_borrow(self)
        }
        #[cfg(not(feature = "parallel"))]
        {
            V::try_borrow(&self)
        }
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{EntitiesView, View, ViewMut, World};

let world = World::new();

let u32s = world.borrow::<View<u32>>();
let (entities, mut usizes) = world.borrow::<(EntitiesView, ViewMut<usize>)>();
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn borrow<'s, V: Borrow<'s>>(&'s self) -> V {
        match self.try_borrow::<V>() {
            Ok(r) => r,
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
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{EntityId, Get, ViewMut, World};

fn sys1((entity, [x, y]): (EntityId, [f32; 2]), mut positions: ViewMut<[f32; 2]>) {
    if let Ok(mut pos) = (&mut positions).get(entity) {
        *pos = [x, y];
    }
}

let world = World::new();

world.try_run_with_data(sys1, (EntityId::dead(), [0., 0.])).unwrap();
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn try_run_with_data<'s, Data, B, R, S: crate::system::System<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> Result<R, error::Run> {
        Ok(s.run((data,), {
            #[cfg(feature = "parallel")]
            {
                S::try_borrow(self)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                S::try_borrow(&self)?
            }
        }))
    }
    #[doc = "Borrows the requested storages and runs the function.  
Data can be passed to the function, this always has to be a single type but you can use a tuple if needed.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{EntityId, Get, ViewMut, World};

fn sys1((entity, [x, y]): (EntityId, [f32; 2]), mut positions: ViewMut<[f32; 2]>) {
    if let Ok(mut pos) = (&mut positions).get(entity) {
        *pos = [x, y];
    }
}

let world = World::new();

world.run_with_data(sys1, (EntityId::dead(), [0., 0.]));
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn run_with_data<'s, Data, B, R, S: crate::system::System<'s, (Data,), B, R>>(
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
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

world
    .try_run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
        // -- snip --
    })
    .unwrap();

let i = world.try_run(sys1).unwrap();
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn try_run<'s, B, R, S: crate::system::System<'s, (), B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        Ok(s.run((), {
            #[cfg(feature = "parallel")]
            {
                S::try_borrow(self)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                S::try_borrow(&self)?
            }
        }))
    }
    #[doc = "Borrows the requested storages and runs the function.  
Unwraps errors.

You can use:
* [View]\\<T\\> for a shared access to `T` storage
* [ViewMut]\\<T\\> for an exclusive access to `T` storage
* [EntitiesView] for a shared access to the entity storage
* [EntitiesViewMut] for an exclusive reference to the entity storage
* [AllStoragesViewMut] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{View, ViewMut, World};

fn sys1(i32s: View<i32>) -> i32 {
    0
}

let world = World::new();

world.run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
    // -- snip --
});

let i = world.run(sys1);
```
[AllStorages]: struct.AllStorages.html
[EntitiesView]: struct.Entities.html
[EntitiesViewMut]: struct.Entities.html
[AllStoragesViewMut]: struct.AllStorages.html
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
    pub fn run<'s, B, R, S: crate::system::System<'s, (), B, R>>(&'s self, s: S) -> R {
        match self.try_run(s) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Modifies the current default workload to `name`.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload did not exist.
    pub fn try_set_default_workload(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> Result<(), error::SetDefaultWorkload> {
        self.scheduler
            .try_borrow_mut()
            .map_err(|_| error::SetDefaultWorkload::Borrow)?
            .set_default(name.into())
    }
    /// Modifies the current default workload to `name`.  
    /// Unwraps errors.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload did not exist.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn set_default_workload(&self, name: impl Into<Cow<'static, str>>) {
        match self.try_set_default_workload(name) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Runs the `name` workload.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    /// - Systems' borrow as they are executed
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload did not exist.
    /// - Storage borrow failed.
    /// - User error returned by system.
    pub fn try_run_workload(&self, name: impl AsRef<str>) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        let batches = scheduler.workload(name.as_ref())?;

        self.try_run_workload_index(&scheduler, batches)
    }
    /// Runs the `name` workload.  
    /// Unwraps error.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    /// - Systems' borrow as they are executed
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Workload did not exist.
    /// - Storage borrow failed.
    /// - User error returned by system.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run_workload(&self, name: impl AsRef<str> + Sync) {
        match self.try_run_workload(name) {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    fn try_run_workload_index(
        &self,
        scheduler: &Scheduler,
        batches: &[Vec<usize>],
    ) -> Result<(), error::RunWorkload> {
        for batch in batches {
            if batch.len() == 1 {
                scheduler.systems[batch[0]](self).map_err(|err| {
                    error::RunWorkload::Run((scheduler.system_names[batch[0]], err))
                })?;
            } else {
                #[cfg(feature = "parallel")]
                {
                    use rayon::prelude::*;

                    batch.into_par_iter().try_for_each(|&index| {
                        (scheduler.systems[index])(self).map_err(|err| {
                            error::RunWorkload::Run((scheduler.system_names[index], err))
                        })
                    })?
                }
                #[cfg(not(feature = "parallel"))]
                {
                    batch.iter().try_for_each(|&index| {
                        (scheduler.systems[index])(self).map_err(|err| {
                            error::RunWorkload::Run((scheduler.system_names[index], err))
                        })
                    })?
                }
            }
        }
        Ok(())
    }
    /// Run the default workload if there is one.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    /// - Systems' borrow as they are executed
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Storage borrow failed.
    /// - User error returned by system.
    pub fn try_run_default(&self) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        if !scheduler.is_empty() {
            self.try_run_workload_index(&scheduler, scheduler.default_workload())?
        }
        Ok(())
    }
    /// Run the default workload if there is one.  
    /// Unwraps error.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    /// - Systems' borrow as they are executed
    ///
    /// ### Errors
    ///
    /// - Scheduler borrow failed.
    /// - Storage borrow failed.
    /// - User error returned by system.
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn run_default(&self) {
        match self.try_run_default() {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    /// Used to create an entity without having to borrow its storage explicitly.  
    /// The entity is only added when [EntityBuilder::try_build] or [EntityBuilder::build] is called.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [EntityBuilder::build]: struct.EntityBuilder.html#method.build
    /// [EntityBuilder::try_build]: struct.EntityBuilder.html#method.try_build
    pub fn try_entity_builder(&self) -> Result<EntityBuilder<'_, (), ()>, error::Borrow> {
        Ok(EntityBuilder::new(self.all_storages.try_borrow()?))
    }
    /// Used to create an entity without having to borrow its storage explicitly.  
    /// The entity is only added when [EntityBuilder::try_build] or [EntityBuilder::build] is called.  
    /// Unwraps error.
    ///
    /// ### Borrows
    ///
    /// - [AllStorages] (shared)
    ///
    /// ### Errors
    ///
    /// - [AllStorages] borrow failed.
    ///
    /// [AllStorages]: struct.AllStorages.html
    /// [EntityBuilder::build]: struct.EntityBuilder.html#method.build
    /// [EntityBuilder::try_build]: struct.EntityBuilder.html#method.try_build
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    #[track_caller]
    pub fn entity_builder(&self) -> EntityBuilder<'_, (), ()> {
        match self.try_entity_builder() {
            Ok(r) => r,
            Err(err) => panic!("{:?}", err),
        }
    }
    // /// Serializes the [World] the way `ser_config` defines it.
    // ///
    // /// ### Borrows
    // ///
    // /// - [AllStorages] (exclusively)
    // ///
    // /// ### Errors
    // ///
    // /// - [AllStorages] borrow failed.
    // /// - Serialization error.
    // /// - Config not implemented. (temporary)
    // ///
    // /// [AllStorages]: struct.AllStorages.html
    // /// [World]: struct.World.html
    // #[cfg(feature = "serde1")]
    // #[cfg_attr(docsrs, doc(cfg(feature = "serde1")))]
    // pub fn serialize<S>(
    //     &self,
    //     ser_config: GlobalSerConfig,
    //     serializer: S,
    // ) -> Result<S::Ok, S::Error>
    // where
    //     S: serde::Serializer,
    //     <S as serde::Serializer>::Ok: 'static,
    // {
    //     if ser_config.same_binary == true
    //         && ser_config.with_entities == true
    //         && ser_config.with_shared == WithShared::PerStorage
    //     {
    //         serializer.serialize_newtype_struct(
    //             "World",
    //             &crate::storage::AllStoragesSerializer {
    //                 all_storages: self
    //                     .all_storages
    //                     .try_borrow_mut()
    //                     .map_err(|err| serde::ser::Error::custom(err))?,
    //                 ser_config,
    //             },
    //         )
    //     } else {
    //         Err(serde::ser::Error::custom(
    //             "ser_config other than default isn't implemented yet",
    //         ))
    //     }
    // }
    // #[cfg(feature = "serde1")]
    // pub fn deserialize<'de, D>(
    //     &self,
    //     de_config: GlobalDeConfig,
    //     deserializer: D,
    // ) -> Result<(), D::Error>
    // where
    //     D: serde::Deserializer<'de>,
    // {
    //     if de_config.existing_entities == ExistingEntities::AsNew
    //         && de_config.with_shared == WithShared::PerStorage
    //     {
    //         Ok(())
    //     } else {
    //         Err(serde::de::Error::custom(
    //             "de_config other than default isn't implemented yet",
    //         ))
    //     }
    // }
    // /// Creates a new [World] from a deserializer the way `de_config` defines it.
    // ///
    // /// ### Errors
    // ///
    // /// - Deserialization error.
    // /// - Config not implemented. (temporary)
    // ///
    // /// [World]: struct.World.html
    // #[cfg(feature = "serde1")]
    // #[cfg_attr(docsrs, doc(cfg(feature = "serde1")))]
    // pub fn new_deserialized<'de, D>(
    //     de_config: GlobalDeConfig,
    //     deserializer: D,
    // ) -> Result<Self, D::Error>
    // where
    //     D: serde::Deserializer<'de>,
    // {
    //     if de_config.existing_entities == ExistingEntities::AsNew
    //         && de_config.with_shared == WithShared::PerStorage
    //     {
    //         let world = World::new();
    //         deserializer.deserialize_struct(
    //             "World",
    //             &["metadata", "storages"],
    //             WorldVisitor {
    //                 all_storages: world
    //                     .all_storages
    //                     .try_borrow_mut()
    //                     .map_err(serde::de::Error::custom)?,
    //                 de_config,
    //             },
    //         )?;
    //         Ok(world)
    //     } else {
    //         Err(serde::de::Error::custom(
    //             "de_config other than default isn't implemented yet",
    //         ))
    //     }
    // }
}

// #[cfg(feature = "serde1")]
// struct WorldVisitor<'a> {
//     all_storages: RefMut<'a, AllStorages>,
//     de_config: GlobalDeConfig,
// }

// #[cfg(feature = "serde1")]
// impl<'de, 'a> serde::de::Visitor<'de> for WorldVisitor<'a> {
//     type Value = ();

//     fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         formatter.write_str("Could not format World")
//     }

//     fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
//     where
//         A: serde::de::MapAccess<'de>,
//     {
//         match map.next_key()? {
//             Some("ser_infos") => (),
//             Some(field) => {
//                 return Err(serde::de::Error::unknown_field(
//                     field,
//                     &["ser_infos", "metadata", "storages"],
//                 ))
//             }
//             None => return Err(serde::de::Error::missing_field("ser_infos")),
//         };

//         let ser_infos: crate::serde_setup::SerInfos = map.next_value()?;

//         if ser_infos.same_binary {
//             let metadata: Vec<(StorageId, usize)>;

//             match map.next_entry()? {
//                 Some(("metadata", types)) => metadata = types,
//                 Some((field, _)) => {
//                     return Err(serde::de::Error::unknown_field(
//                         field,
//                         &["ser_infos", "metadata", "storages"],
//                     ))
//                 }
//                 None => return Err(serde::de::Error::missing_field("metadata")),
//             }

//             match map.next_key_seed(core::marker::PhantomData)? {
//                 Some("storages") => (),
//                 Some(field) => {
//                     return Err(serde::de::Error::unknown_field(
//                         field,
//                         &["ser_infos", "metadata", "storages"],
//                     ))
//                 }
//                 None => return Err(serde::de::Error::missing_field("storages")),
//             }

//             map.next_value_seed(StoragesSeed {
//                 metadata,
//                 all_storages: self.all_storages,
//                 de_config: self.de_config,
//             })?;
//         } else {
//             todo!()
//         }

//         Ok(())
//     }
// }

// #[cfg(feature = "serde1")]
// struct StoragesSeed<'all> {
//     metadata: Vec<(StorageId, usize)>,
//     all_storages: RefMut<'all, AllStorages>,
//     de_config: GlobalDeConfig,
// }

// #[cfg(feature = "serde1")]
// impl<'de> serde::de::DeserializeSeed<'de> for StoragesSeed<'_> {
//     type Value = ();

//     fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         struct StoragesVisitor<'all> {
//             metadata: Vec<(StorageId, usize)>,
//             all_storages: RefMut<'all, AllStorages>,
//             de_config: GlobalDeConfig,
//         }

//         impl<'de> serde::de::Visitor<'de> for StoragesVisitor<'_> {
//             type Value = ();

//             fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//                 formatter.write_str("storages value")
//             }

//             fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
//             where
//                 A: serde::de::SeqAccess<'de>,
//             {
//                 let storages = self.all_storages.storages();

//                 for (i, (storage_id, deserialize_ptr)) in self.metadata.into_iter().enumerate() {
//                     let storage: &mut Storage =
//                         &mut storages.entry(storage_id).or_insert_with(|| {
//                             let deserialize =
//                                 unsafe { crate::unknown_storage::deserialize_fn(deserialize_ptr) };

//                             let mut sparse_set = crate::sparse_set::SparseSet::<u8>::new();
//                             sparse_set.metadata.serde = Some(crate::sparse_set::SerdeInfos {
//                                 serialization:
//                                     |sparse_set: &crate::sparse_set::SparseSet<u8>,
//                                     ser_config: GlobalSerConfig,
//                                     serializer: &mut dyn crate::erased_serde::Serializer| {
//                                         crate::erased_serde::Serialize::erased_serialize(
//                                             &crate::sparse_set::SparseSetSerializer {
//                                                 sparse_set: &sparse_set,
//                                                 ser_config,
//                                             },
//                                             serializer,
//                                         )
//                                     },
//                                 deserialization: deserialize,
//                                 with_shared: true,
//                                 identifier: None,
//                             });

//                             Storage(Box::new(AtomicRefCell::new(sparse_set, None, true)))
//                         });

//                     if seq
//                         .next_element_seed(crate::storage::StorageDeserializer {
//                             storage,
//                             de_config: self.de_config,
//                         })?
//                         .is_none()
//                     {
//                         return Err(serde::de::Error::invalid_length(i, &"more storages"));
//                     }
//                 }

//                 Ok(())
//             }
//         }

//         deserializer.deserialize_seq(StoragesVisitor {
//             metadata: self.metadata,
//             all_storages: self.all_storages,
//             de_config: self.de_config,
//         })
//     }
// }

// #[cfg(feature = "serde1")]
// struct ExistingWorldVisitor<'a> {
//     all_storages: RefMut<'a, AllStorages>,
//     de_config: GlobalDeConfig,
// }
