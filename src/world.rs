mod builder;
mod run_batches;

pub use builder::WorldBuilder;

use crate::all_storages::{AllStorages, CustomStorageAccess, TupleDeleteAny, TupleRetainStorage};
use crate::atomic_refcell::{ARef, ARefMut, AtomicRefCell};
use crate::borrow::WorldBorrow;
use crate::component::{Component, Unique};
use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::error;
use crate::get_component::GetComponent;
use crate::get_unique::GetUnique;
use crate::iter::{ShiperatorCaptain, ShiperatorSailor};
use crate::iter_component::{into_iter, IntoIterRef, IterComponent};
use crate::memory_usage::WorldMemoryUsage;
use crate::r#mut::Mut;
use crate::reserve::BulkEntityIter;
use crate::scheduler::info::WorkloadsInfo;
use crate::scheduler::{AsLabel, Batches, Label, Scheduler};
use crate::sparse_set::{BulkAddEntity, TupleAddComponent, TupleDelete, TupleRemove};
use crate::storage::{Storage, StorageId};
use crate::system::System;
use crate::tracking::{TrackingTimestamp, TupleTrack};
use crate::views::EntitiesViewMut;
use alloc::boxed::Box;
use alloc::format;
use alloc::sync::Arc;
use core::sync::atomic::AtomicU64;

/// `World` contains all data this library will manipulate.
pub struct World {
    pub(crate) all_storages: AtomicRefCell<AllStorages>,
    pub(crate) scheduler: AtomicRefCell<Scheduler>,
    counter: Arc<AtomicU64>,
    #[cfg(feature = "parallel")]
    thread_pool: Option<rayon::ThreadPool>,
}

#[cfg(feature = "std")]
impl Default for World {
    /// Creates an empty `World`.
    fn default() -> Self {
        let counter = Arc::new(AtomicU64::new(1));
        World {
            #[cfg(not(feature = "thread_local"))]
            all_storages: AtomicRefCell::new(AllStorages::new(counter.clone())),
            #[cfg(feature = "thread_local")]
            all_storages: AtomicRefCell::new_non_send(
                AllStorages::new(counter.clone()),
                Arc::new(crate::std_thread_id_generator),
            ),
            scheduler: AtomicRefCell::new(Default::default()),
            counter,
            #[cfg(feature = "parallel")]
            thread_pool: None,
        }
    }
}

impl World {
    /// Creates an empty `World`.
    #[cfg(feature = "std")]
    pub fn new() -> World {
        Default::default()
    }
    /// Removes the local [`ThreadPool`](rayon::ThreadPool).
    #[cfg(feature = "parallel")]
    pub fn remove_local_thread_pool(&mut self) -> Option<rayon::ThreadPool> {
        self.thread_pool.take()
    }
    /// Adds a new unique storage, unique storages store a single value.
    /// To access a unique storage value, use [`UniqueView`] or [`UniqueViewMut`].
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Panics
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Unique, UniqueView, World};
    ///
    /// #[derive(Unique)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// world.add_unique(U32(0));
    ///
    /// let i = world.borrow::<UniqueView<U32>>().unwrap();
    /// assert_eq!(i.0, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    #[track_caller]
    pub fn add_unique<T: Send + Sync + Unique>(&self, component: T) {
        self.all_storages.borrow().unwrap().add_unique(component);
    }
    /// Adds a new unique storage, unique storages store a single value.
    /// To access a `!Send` unique storage value, use [`NonSend`] with [`UniqueView`] or [`UniqueViewMut`].
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Panics
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSend, Unique, UniqueView, World};
    ///
    /// #[derive(Unique)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// // I'm using `u32` here but imagine it's a `!Send` type
    /// world.add_unique_non_send(U32(0));
    ///
    /// let i = world.borrow::<NonSend<UniqueView<U32>>>().unwrap();
    /// assert_eq!(i.0, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSend`]: crate::NonSend
    #[cfg(feature = "thread_local")]
    #[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
    #[track_caller]
    pub fn add_unique_non_send<T: Sync + Unique>(&self, component: T) {
        self.all_storages
            .borrow()
            .unwrap()
            .add_unique_non_send(component);
    }
    /// Adds a new unique storage, unique storages store a single value.
    /// To access a `!Sync` unique storage value, use [`NonSync`] with [`UniqueView`] or [`UniqueViewMut`].
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Panics
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSync, Unique, UniqueView, World};
    ///
    /// #[derive(Unique)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// // I'm using `u32` here but imagine it's a `!Sync` type
    /// world.add_unique_non_sync(U32(0));
    ///
    /// let i = world.borrow::<NonSync<UniqueView<U32>>>().unwrap();
    /// assert_eq!(i.0, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSync`]: crate::NonSync
    #[cfg(feature = "thread_local")]
    #[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
    #[track_caller]
    pub fn add_unique_non_sync<T: Send + Unique>(&self, component: T) {
        self.all_storages
            .borrow()
            .unwrap()
            .add_unique_non_sync(component);
    }
    /// Adds a new unique storage, unique storages store a single value.
    /// To access a `!Send + !Sync` unique storage value, use [`NonSendSync`] with [`UniqueView`] or [`UniqueViewMut`].
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Panics
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSendSync, Unique, UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// #[derive(Unique)]
    /// struct U32(u32);
    ///
    /// // I'm using `u32` here but imagine it's a `!Send + !Sync` type
    /// world.add_unique_non_send_sync(U32(0));
    ///
    /// let i = world.borrow::<NonSendSync<UniqueView<U32>>>().unwrap();
    /// assert_eq!(i.0, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSendSync`]: crate::NonSync
    #[cfg(feature = "thread_local")]
    #[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
    #[track_caller]
    pub fn add_unique_non_send_sync<T: Unique>(&self, component: T) {
        self.all_storages
            .borrow()
            .unwrap()
            .add_unique_non_send_sync(component);
    }
    /// Removes a unique storage.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    /// - `Unique<T>` storage (exclusive)
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    /// - `Unique<T>` storage borrow failed.
    /// - `Unique<T>` storage did not exist.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Unique, UniqueView, World};
    ///
    /// #[derive(Unique, Debug)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// world.add_unique(U32(0));
    ///
    /// let i = world.remove_unique::<U32>().unwrap();
    /// assert_eq!(i.0, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    pub fn remove_unique<T: Unique>(&self) -> Result<T, error::UniqueRemove> {
        self.all_storages
            .borrow()
            .map_err(|_| error::UniqueRemove::AllStorages)?
            .remove_unique::<T>()
    }
    #[doc = "Borrows the requested storages, if they don't exist they'll get created.
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.

### Example
```
use shipyard::{Component, EntitiesView, View, ViewMut, World};

#[derive(Component)]
struct U32(u32);

#[derive(Component)]
struct USIZE(usize);

let world = World::new();

let u32s = world.borrow::<View<U32>>().unwrap();
let (entities, mut usizes) = world
    .borrow::<(EntitiesView, ViewMut<USIZE>)>()
    .unwrap();
```
[AllStorages]: crate::AllStorages
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[AllStoragesViewMut]: crate::AllStorages
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    pub fn borrow<V: WorldBorrow>(&self) -> Result<V::WorldView<'_>, error::GetStorage> {
        let current = self.get_current();

        V::world_borrow(self, None, current)
    }
    #[doc = "Borrows the requested storages, runs the function and evaluates to the function's return value.
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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Panics

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{Component, EntityId, Get, ViewMut, World};

#[derive(Component)]
struct Position([f32; 2]);

fn sys1((entity, [x, y]): (EntityId, [f32; 2]), mut positions: ViewMut<Position>) {
    if let Ok(mut pos) = (&mut positions).get(entity) {
        pos.0 = [x, y];
    }
}

let world = World::new();

world.run_with_data(sys1, (EntityId::dead(), [0., 0.]));
```
[AllStorages]: crate::AllStorages
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[AllStoragesViewMut]: crate::AllStorages
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    #[track_caller]
    pub fn run_with_data<Data, B, S: System<(Data,), B>>(
        &self,
        system: S,
        data: Data,
    ) -> S::Return {
        #[cfg(feature = "tracing")]
        let system_span = tracing::info_span!("system", name = ?core::any::type_name::<S>());
        #[cfg(feature = "tracing")]
        let _system_span = system_span.enter();

        system
            .run((data,), self)
            .map_err(error::Run::GetStorage)
            .unwrap()
    }
    #[doc = "Borrows the requested storages, runs the function and evaluates to the function's return value.

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

- [AllStorages] (exclusive) when requesting [AllStoragesViewMut]
- [AllStorages] (shared) + storage (exclusive or shared) for all other views

### Panics

- [AllStorages] borrow failed.
- Storage borrow failed.
- Unique storage did not exist.
- Error returned by user.

### Example
```
use shipyard::{Component, View, ViewMut, World};

#[derive(Component)]
struct I32(i32);

#[derive(Component)]
struct USIZE(usize);

#[derive(Component)]
struct U32(u32);

fn sys1(i32s: View<I32>) -> i32 {
    0
}

let world = World::new();

world
    .run(|usizes: View<USIZE>, mut u32s: ViewMut<U32>| {
        // -- snip --
    });

let i = world.run(sys1);
```
[AllStorages]: crate::AllStorages
[EntitiesView]: crate::Entities
[EntitiesViewMut]: crate::Entities
[AllStoragesViewMut]: crate::AllStorages
[World]: crate::World
[View]: crate::View
[ViewMut]: crate::ViewMut
[UniqueView]: crate::UniqueView
[UniqueViewMut]: crate::UniqueViewMut"]
    #[cfg_attr(feature = "thread_local", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(feature = "thread_local", doc = "[NonSendSync]: crate::NonSendSync")]
    #[track_caller]
    pub fn run<B, S: System<(), B>>(&self, system: S) -> S::Return {
        #[cfg(feature = "tracing")]
        let system_span = tracing::info_span!("system", name = ?core::any::type_name::<S>());
        #[cfg(feature = "tracing")]
        let _system_span = system_span.enter();

        system
            .run((), self)
            .map_err(error::Run::GetStorage)
            .unwrap()
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
    pub fn set_default_workload<T>(
        &self,
        name: impl AsLabel<T>,
    ) -> Result<(), error::SetDefaultWorkload> {
        self.scheduler
            .borrow_mut()
            .map_err(|_| error::SetDefaultWorkload::Borrow)?
            .set_default(name.as_label())
    }
    /// Changes the name of a workload if it exists.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (exclusive)
    ///
    /// ### Panics
    ///
    /// - Scheduler borrow failed.
    #[track_caller]
    pub fn rename_workload<T, U>(&self, old_name: impl AsLabel<T>, new_name: impl AsLabel<U>) {
        let old_label = old_name.as_label();
        let new_label = new_name.as_label();

        self.scheduler
            .borrow_mut()
            .unwrap()
            .rename(&old_label, Box::new(new_label));
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
    pub fn run_workload<T>(&self, label: impl AsLabel<T>) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        let label = label.as_label();
        let batches = scheduler.workload(&*label)?;

        self.run_batches(
            &scheduler.systems,
            &scheduler.system_names,
            batches,
            &*label,
        )
    }
    /// Returns `true` if the world contains the `name` workload.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    ///
    /// ### Panics
    ///
    /// - Scheduler borrow failed.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Workload, World};
    ///
    /// let world = World::new();
    ///
    /// Workload::new("foo").add_to_world(&world).unwrap();
    ///
    /// assert!(world.contains_workload("foo"));
    /// assert!(!world.contains_workload("bar"));
    /// ```
    #[track_caller]
    pub fn contains_workload<T>(&self, name: impl AsLabel<T>) -> bool {
        let label = name.as_label();

        self.scheduler.borrow().unwrap().contains_workload(&*label)
    }
    #[allow(clippy::type_complexity)]
    pub(crate) fn run_batches(
        &self,
        systems: &[Box<dyn Fn(&World) -> Result<(), error::Run> + Send + Sync + 'static>],
        system_names: &[Box<dyn Label>],
        batches: &Batches,
        workload_name: &dyn Label,
    ) -> Result<(), error::RunWorkload> {
        if let Some(run_if) = &batches.run_if {
            if !run_if
                .run(self)
                .map_err(|err| error::RunWorkload::Run((workload_name.dyn_clone(), err)))?
            {
                return Ok(());
            }
        }

        #[cfg(feature = "parallel")]
        {
            self.run_batches_parallel(systems, system_names, batches, workload_name)
        }

        #[cfg(not(feature = "parallel"))]
        {
            self.run_batches_sequential(systems, system_names, batches, workload_name)
        }
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
    pub fn run_default_workload(&self) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        if !scheduler.is_empty() {
            self.run_batches(
                &scheduler.systems,
                &scheduler.system_names,
                scheduler.default_workload(),
                &scheduler.default,
            )?
        }
        Ok(())
    }
    /// Returns a `Ref<&AllStorages>`, used to implement custom storages.
    /// To borrow `AllStorages` you should use `borrow` or `run` with `AllStoragesViewMut`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed.
    pub fn all_storages(&self) -> Result<ARef<'_, &'_ AllStorages>, error::Borrow> {
        self.all_storages.borrow()
    }
    /// Returns a `RefMut<&mut AllStorages>`, used to implement custom storages.
    /// To borrow `AllStorages` you should use `borrow` or `run` with `AllStoragesViewMut`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed.
    pub fn all_storages_mut(&self) -> Result<ARefMut<'_, &'_ mut AllStorages>, error::Borrow> {
        self.all_storages.borrow_mut()
    }
    /// Inserts a custom storage to the `World`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed exclusively.
    pub fn add_custom_storage<S: 'static + Storage + Send + Sync>(
        &self,
        storage_id: StorageId,
        storage: S,
    ) -> Result<(), error::Borrow> {
        let _ = self
            .all_storages
            .borrow()?
            .custom_storage_or_insert_by_id(storage_id, || storage);

        Ok(())
    }

    /// Increments the current tracking cycle and returns the previous value.
    #[inline]
    pub(crate) fn get_current(&self) -> TrackingTimestamp {
        TrackingTimestamp::new(
            self.counter
                .fetch_add(1, core::sync::atomic::Ordering::Acquire),
        )
    }

    /// Returns a timestamp used to clear tracking information.
    pub fn get_tracking_timestamp(&self) -> TrackingTimestamp {
        TrackingTimestamp::new(self.counter.load(core::sync::atomic::Ordering::Acquire))
    }
}

impl World {
    /// Creates a new entity with the components passed as argument and returns its `EntityId`.
    /// `component` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity0 = world.add_entity((U32(0),));
    /// let entity1 = world.add_entity((U32(1), USIZE(11)));
    /// ```
    #[inline]
    pub fn add_entity<C: TupleAddComponent>(&mut self, component: C) -> EntityId {
        self.all_storages.get_mut().add_entity(component)
    }
    /// Creates multiple new entities and returns an iterator yielding the new `EntityId`s.
    /// `source` must always yield a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let new_entities = world.bulk_add_entity((10..20).map(|i| (U32(i as u32), USIZE(i))));
    /// ```
    #[inline]
    pub fn bulk_add_entity<T: BulkAddEntity>(&mut self, source: T) -> BulkEntityIter<'_> {
        self.all_storages.get_mut().bulk_add_entity(source)
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
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// // make an empty entity
    /// let entity = world.add_entity(());
    ///
    /// world.add_component(entity, (U32(0),));
    /// // entity already had a `U32` component so it will be replaced
    /// world.add_component(entity, (U32(1), USIZE(11)));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_component<C: TupleAddComponent>(&mut self, entity: EntityId, component: C) {
        self.all_storages.get_mut().add_component(entity, component)
    }
    /// Deletes components from an entity. As opposed to `remove`, `delete` doesn't return anything.
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((U32(0), USIZE(1)));
    ///
    /// world.delete_component::<(U32,)>(entity);
    /// ```
    #[inline]
    pub fn delete_component<C: TupleDelete>(&mut self, entity: EntityId) {
        self.all_storages.get_mut().delete_component::<C>(entity)
    }
    /// Removes components from an entity.
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((U32(0), USIZE(1)));
    ///
    /// let (i,) = world.remove::<(U32,)>(entity);
    /// assert_eq!(i, Some(U32(0)));
    /// ```
    #[inline]
    pub fn remove<C: TupleRemove>(&mut self, entity: EntityId) -> C::Out {
        self.all_storages.get_mut().remove::<C>(entity)
    }
    /// Deletes an entity with all its components. Returns true if the entity were alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((U32(0), USIZE(1)));
    ///
    /// assert!(world.delete_entity(entity));
    /// ```
    #[inline]
    pub fn delete_entity(&mut self, entity: EntityId) -> bool {
        self.all_storages.get_mut().delete_entity(entity)
    }
    /// Deletes all components of an entity without deleting the entity.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((U32(0), USIZE(1)));
    ///
    /// world.strip(entity);
    /// ```
    #[inline]
    pub fn strip(&mut self, entity: EntityId) {
        self.all_storages.get_mut().strip(entity);
    }
    /// Deletes all entities with any of the given components.
    /// The storage's type has to be used and not the component.
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, sparse_set::SparseSet, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// #[derive(Component)]
    /// struct STR(&'static str);
    ///
    /// let mut world = World::new();
    ///
    /// let entity0 = world.add_entity((U32(0),));
    /// let entity1 = world.add_entity((USIZE(1),));
    /// let entity2 = world.add_entity((STR("2"),));
    ///
    /// // deletes `entity2`
    /// world.delete_any::<SparseSet<STR>>();
    /// // deletes `entity0` and `entity1`
    /// world.delete_any::<(SparseSet<U32>, SparseSet<USIZE>)>();
    /// ```
    #[inline]
    pub fn delete_any<S: TupleDeleteAny>(&mut self) {
        self.all_storages.get_mut().delete_any::<S>();
    }
    /// Deletes all components of an entity except the ones passed in `S`.
    /// The storage's type has to be used and not the component.
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, sparse_set::SparseSet, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((U32(0), USIZE(1)));
    ///
    /// world.retain_storage::<SparseSet<U32>>(entity);
    /// ```
    #[inline]
    pub fn retain_storage<S: TupleRetainStorage>(&mut self, entity: EntityId) {
        self.all_storages.get_mut().retain_storage::<S>(entity);
    }
    /// Same as `retain_storage` but uses `StorageId` and not generics.
    /// You should only use this method if you use a custom storage with a runtime id.
    #[inline]
    pub fn retain_storage_by_id(&mut self, entity: EntityId, excluded_storage: &[StorageId]) {
        self.all_storages
            .get_mut()
            .retain_storage_by_id(entity, excluded_storage);
    }
    /// Deletes all entities and components in the `World`.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// world.clear();
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.all_storages.get_mut().clear();
    }
    /// Clear all deletion and removal tracking data.
    pub fn clear_all_removed_and_deleted(&mut self) {
        self.all_storages.get_mut().clear_all_removed_and_deleted()
    }
    /// Clear all deletion and removal tracking data older than some timestamp.
    pub fn clear_all_removed_and_deleted_older_than_timestamp(
        &mut self,
        timestamp: TrackingTimestamp,
    ) {
        self.all_storages
            .get_mut()
            .clear_all_removed_and_deleted_older_than_timestamp(timestamp)
    }
    /// Make the given entity alive.
    /// Does nothing if an entity with a greater generation is already at this index.
    /// Returns `true` if the entity is successfully spawned.
    #[inline]
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        self.all_storages.get_mut().spawn(entity)
    }

    /// Deletes all components for which `f(id, &component)` returns `false`.
    ///
    /// # Panics
    ///
    /// - Storage borrow failed.
    pub fn retain<T: Component + Send + Sync>(&mut self, f: impl FnMut(EntityId, &T) -> bool) {
        self.all_storages.get_mut().retain(f);
    }

    /// Deletes all components for which `f(id, Mut<component>)` returns `false`.
    ///
    /// # Panics
    ///
    /// - Storage borrow failed.
    pub fn retain_mut<T: Component + Send + Sync>(
        &mut self,
        f: impl FnMut(EntityId, Mut<'_, T>) -> bool,
    ) {
        self.all_storages.get_mut().retain_mut(f);
    }

    /// Displays storages memory information.
    pub fn memory_usage(&self) -> WorldMemoryUsage<'_> {
        WorldMemoryUsage(self)
    }
    /// Returns a list of workloads and all information related to them.
    ///
    /// ### Borrows
    ///
    /// - Scheduler (shared)
    ///
    /// ### Panics
    ///
    /// - Scheduler borrow failed.
    #[track_caller]
    pub fn workloads_info(&self) -> WorkloadsInfo {
        let scheduler = self.scheduler.borrow().unwrap();

        WorkloadsInfo(
            scheduler
                .workloads_info
                .iter()
                .map(|(name, workload_info)| (format!("{:?}", name), workload_info.clone()))
                .collect(),
        )
    }

    /// Enable insertion tracking for the given components.
    pub fn track_insertion<T: TupleTrack>(&mut self) -> &mut World {
        self.all_storages.get_mut().track_insertion::<T>();
        self
    }

    /// Enable modification tracking for the given components.
    pub fn track_modification<T: TupleTrack>(&mut self) -> &mut World {
        self.all_storages.get_mut().track_modification::<T>();
        self
    }

    /// Enable deletion tracking for the given components.
    pub fn track_deletion<T: TupleTrack>(&mut self) -> &mut World {
        self.all_storages.get_mut().track_deletion::<T>();
        self
    }

    /// Enable removal tracking for the given components.
    pub fn track_removal<T: TupleTrack>(&mut self) -> &mut World {
        self.all_storages.get_mut().track_removal::<T>();
        self
    }

    /// Enable insertion, deletion and removal tracking for the given components.
    pub fn track_all<T: TupleTrack>(&mut self) {
        self.all_storages.get_mut().track_all::<T>();
    }

    #[doc = "Retrieve components of `entity`.

Multiple components can be queried at the same time using a tuple.

You can use:
* `&T` for a shared access to `T` component
* `&mut T` for an exclusive access to `T` component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared) + storage (exclusive or shared)

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.
- Entity does not have the component.

### Example
```
use shipyard::{Component, World};

#[derive(Component, Debug, PartialEq, Eq)]
struct U32(u32);

#[derive(Component, Debug, PartialEq, Eq)]
struct USIZE(usize);

let mut world = World::new();

let entity = world.add_entity((USIZE(0), U32(1)));

let (i, j) = world.get::<(&USIZE, &mut U32)>(entity).unwrap();

assert!(*i == &USIZE(0));
assert!(*j == &U32(1));
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    pub fn get<T: GetComponent>(
        &self,
        entity: EntityId,
    ) -> Result<T::Out<'_>, error::GetComponent> {
        let (all_storages, all_borrow) = unsafe {
            ARef::destructure(
                self.all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        let current = self.get_current();

        T::get(all_storages, Some(all_borrow), current, entity)
    }

    #[doc = "Retrieve a unique component.

You can use:
* `&T` for a shared access to `T` unique component
* `&mut T` for an exclusive access to `T` unique component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` unique component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` unique component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` unique component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` unique component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared) + storage (exclusive or shared)

### Errors

- [AllStorages] borrow failed.
- Storage borrow failed.

### Example
```
use shipyard::{Unique, World};

#[derive(Unique, Debug, PartialEq, Eq)]
struct U32(u32);

let mut world = World::new();

world.add_unique(U32(0));

let i = world.get_unique::<&U32>().unwrap();

assert!(*i == U32(0));
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    pub fn get_unique<T: GetUnique>(&self) -> Result<T::Out<'_>, error::GetStorage> {
        let (all_storages, all_borrow) = unsafe {
            ARef::destructure(
                self.all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        T::get_unique(all_storages, Some(all_borrow))
    }

    #[doc = "Iterate components.

Multiple components can be iterated at the same time using a tuple.

You can use:
* `&T` for a shared access to `T` component
* `&mut T` for an exclusive access to `T` component"]
    #[cfg_attr(
        all(feature = "thread_local", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"thread_local\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "thread_local"),
        doc = "* [NonSend]<&T> for a shared access to a `T` component where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send`
* [NonSync]<&T> for a shared access to a `T` component where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Sync`
* [NonSendSync]<&T> for a shared access to a `T` component where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` component where `T` isn't `Send` nor `Sync`"
    )]
    #[cfg_attr(
        not(feature = "thread_local"),
        doc = "* NonSend: must activate the *thread_local* feature
* NonSync: must activate the *thread_local* feature
* NonSendSync: must activate the *thread_local* feature"
    )]
    #[doc = "
### Borrows

- [AllStorages] (shared)

### Panics

- [AllStorages] borrow failed.

### Example
```
use shipyard::{Component, World};

#[derive(Component, Debug, PartialEq, Eq)]
struct U32(u32);

#[derive(Component, Debug, PartialEq, Eq)]
struct USIZE(usize);

let mut world = World::new();

let entity = world.add_entity((USIZE(0), U32(1)));

let mut iter = world.iter::<(&USIZE, &mut U32)>();

for (i, j) in &mut iter {
    // <-- SNIP -->
}
```"]
    #[cfg_attr(
        feature = "thread_local",
        doc = "[NonSend]: crate::NonSend
[NonSync]: crate::NonSync
[NonSendSync]: crate::NonSendSync"
    )]
    #[inline]
    #[track_caller]
    pub fn iter<'a, T: IterComponent>(&'a self) -> IntoIterRef<'a, T>
    where
        <T as IterComponent>::Shiperator<'a>: ShiperatorCaptain + ShiperatorSailor,
    {
        let (all_storages, all_borrow) = unsafe {
            ARef::destructure(
                self.all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)
                    .unwrap(),
            )
        };

        let current = self.get_current();

        into_iter(all_storages, Some(all_borrow), current).unwrap()
    }

    /// Sets the on entity deletion callback.
    ///
    /// ### Borrows
    ///
    /// - AllStorages (shared)
    /// - Entities (exclusive)
    ///
    /// ### Panics
    ///
    /// - AllStorages borrow failed.
    /// - Entities borrow failed.
    #[track_caller]
    pub fn on_deletion(&self, f: impl FnMut(EntityId) + Send + Sync + 'static) {
        let mut entities = self.borrow::<EntitiesViewMut<'_>>().unwrap();

        entities.on_deletion(f);
    }

    /// Returns true if entity matches a living entity.
    pub fn is_entity_alive(&mut self, entity: EntityId) -> bool {
        self.all_storages
            .get_mut()
            .exclusive_storage_mut::<Entities>()
            .unwrap()
            .is_alive(entity)
    }

    /// Moves an entity from a `World` to another.
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let mut world1 = World::new();
    /// let mut world2 = World::new();
    ///
    /// let entity = world1.add_entity(USIZE(1));
    ///
    /// world1.move_entity(&mut world2, entity);
    ///
    /// assert!(!world1.is_entity_alive(entity));
    /// assert_eq!(world2.get::<&USIZE>(entity).as_deref(), Ok(&&USIZE(1)));
    /// ```
    #[inline]
    #[track_caller]
    pub fn move_entity(&mut self, other: &mut World, entity: EntityId) {
        let other_all_storages = other.all_storages.get_mut();

        self.all_storages
            .get_mut()
            .move_entity(other_all_storages, entity);
    }

    /// Moves all components from an entity to another in another `World`.
    ///
    /// ```
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let mut world1 = World::new();
    /// let mut world2 = World::new();
    ///
    /// let from = world1.add_entity(USIZE(1));
    /// let to = world2.add_entity(());
    ///
    /// world1.move_components(&mut world2, from, to);
    ///
    /// assert!(world1.get::<&USIZE>(from).is_err());
    /// assert_eq!(world2.get::<&USIZE>(to).as_deref(), Ok(&&USIZE(1)));
    /// ```
    #[inline]
    pub fn move_components(&mut self, other: &mut World, from: EntityId, to: EntityId) {
        let other_all_storages = other.all_storages.get_mut();

        self.all_storages
            .get_mut()
            .move_components(other_all_storages, from, to);
    }
}

impl core::fmt::Debug for World {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_tuple("World");

        if let Ok(all_storages) = self.all_storages.borrow() {
            debug_struct.field(&*all_storages);
        } else {
            debug_struct.field(&"Could not borrow AllStorages");
        }

        if let Ok(scheduler) = self.scheduler.borrow() {
            debug_struct.field(&*scheduler);
        } else {
            debug_struct.field(&"Could not borrow Scheduler");
        }

        debug_struct.finish()
    }
}

impl core::fmt::Debug for WorldMemoryUsage<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Ok(all_storages) = self.0.all_storages.borrow() {
            all_storages.memory_usage().fmt(f)
        } else {
            f.write_str("Could not borrow AllStorages")
        }
    }
}
