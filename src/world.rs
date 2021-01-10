use crate::all_storages::{AllStorages, DeleteAny, Retain};
use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::borrow::Borrow;
use crate::entity_id::EntityId;
use crate::error;
use crate::reserve::BulkEntityIter;
use crate::scheduler::{Batches, Scheduler};
use crate::sparse_set::{AddComponent, BulkAddEntity, DeleteComponent, Remove};
use crate::storage::StorageId;
use crate::unknown_storage::UnknownStorage;
use alloc::borrow::Cow;

/// `World` contains all data this library will manipulate.
pub struct World {
    pub(crate) all_storages: AtomicRefCell<AllStorages>,
    pub(crate) scheduler: AtomicRefCell<Scheduler>,
}

impl Default for World {
    /// Creates an empty `World`.
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
    /// Creates an empty `World`.
    pub fn new() -> Self {
        Default::default()
    }
    /// Adds a new unique storage, unique storages store a single value.  
    /// To access a unique storage value, use [`UniqueView`] or [`UniqueViewMut`].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// world.add_unique(0u32).unwrap();
    ///
    /// let i = world.borrow::<UniqueView<u32>>().unwrap();
    /// assert_eq!(*i, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) -> Result<(), error::Borrow> {
        self.all_storages.try_borrow()?.add_unique(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store a single value.  
    /// To access a `!Send` unique storage value, use [`NonSend`] with [`UniqueView`] or [`UniqueViewMut`].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSend, UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// // I'm using `u32` here but imagine it's a `!Send` type
    /// world.add_unique_non_send(0u32).unwrap();
    ///
    /// let i = world.borrow::<NonSend<UniqueView<u32>>>().unwrap();
    /// assert_eq!(**i, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSend`]: crate::NonSend
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    pub fn add_unique_non_send<T: 'static + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_send(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store a single value.  
    /// To access a `!Sync` unique storage value, use [`NonSync`] with [`UniqueView`] or [`UniqueViewMut`].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSync, UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// // I'm using `u32` here but imagine it's a `!Sync` type
    /// world.add_unique_non_sync(0u32).unwrap();
    ///
    /// let i = world.borrow::<NonSync<UniqueView<u32>>>().unwrap();
    /// assert_eq!(**i, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSync`]: crate::NonSync
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    pub fn add_unique_non_sync<T: 'static + Send>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_sync(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store a single value.  
    /// To access a `!Send + !Sync` unique storage value, use [`NonSendSync`] with [`UniqueView`] or [`UniqueViewMut`].  
    /// Does nothing if the storage already exists.
    ///
    /// ### Borrows
    ///
    /// - [`AllStorages`] (shared)
    ///
    /// ### Errors
    ///
    /// - [`AllStorages`] borrow failed.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{NonSendSync, UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// // I'm using `u32` here but imagine it's a `!Send + !Sync` type
    /// world.add_unique_non_send_sync(0u32).unwrap();
    ///
    /// let i = world.borrow::<NonSendSync<UniqueView<u32>>>().unwrap();
    /// assert_eq!(**i, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    /// [`UniqueView`]: crate::UniqueView
    /// [`UniqueViewMut`]: crate::UniqueViewMut
    /// [`NonSendSync`]: crate::NonSync
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow()?
            .add_unique_non_send_sync(component);
        Ok(())
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
    /// use shipyard::{UniqueView, World};
    ///
    /// let world = World::new();
    ///
    /// world.add_unique(0u32).unwrap();
    ///
    /// let i = world.remove_unique::<u32>().unwrap();
    /// assert_eq!(i, 0);
    /// ```
    ///
    /// [`AllStorages`]: crate::AllStorages
    pub fn remove_unique<T: 'static>(&self) -> Result<T, error::UniqueRemove> {
        self.all_storages
            .try_borrow()
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

let u32s = world.borrow::<View<u32>>().unwrap();
let (entities, mut usizes) = world
    .borrow::<(EntitiesView, ViewMut<usize>)>()
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
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: crate::NonSendSync"
    )]
    pub fn borrow<'s, V: Borrow<'s>>(&'s self) -> Result<V, error::GetStorage> {
        V::try_borrow(self)
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

world.run_with_data(sys1, (EntityId::dead(), [0., 0.])).unwrap();
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
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: crate::NonSendSync"
    )]
    pub fn run_with_data<'s, Data, B, R, S: crate::system::System<'s, (Data,), B, R>>(
        &'s self,
        s: S,
        data: Data,
    ) -> Result<R, error::Run> {
        Ok(s.run((data,), S::try_borrow(self)?))
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
    .run(|usizes: View<usize>, mut u32s: ViewMut<u32>| {
        // -- snip --
    })
    .unwrap();

let i = world.run(sys1).unwrap();
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
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: crate::NonSend")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: crate::NonSync")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: crate::NonSendSync"
    )]
    pub fn run<'s, B, R, S: crate::system::System<'s, (), B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        Ok(s.run((), S::try_borrow(self)?))
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
    pub fn set_default_workload(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> Result<(), error::SetDefaultWorkload> {
        self.scheduler
            .try_borrow_mut()
            .map_err(|_| error::SetDefaultWorkload::Borrow)?
            .set_default(name.into())
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
    pub fn run_workload(&self, name: impl AsRef<str>) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        let batches = scheduler.workload(name.as_ref())?;

        self.run_workload_index(&scheduler, batches)
    }
    fn run_workload_index(
        &self,
        scheduler: &Scheduler,
        batches: &Batches,
    ) -> Result<(), error::RunWorkload> {
        #[cfg(feature = "parallel")]
        {
            for batch in &batches.parallel {
                if batch.len() == 1 {
                    scheduler.systems[batch[0]](self).map_err(|err| {
                        error::RunWorkload::Run((scheduler.system_names[batch[0]], err))
                    })?;
                } else {
                    use rayon::prelude::*;

                    batch.into_par_iter().try_for_each(|&index| {
                        (scheduler.systems[index])(self).map_err(|err| {
                            error::RunWorkload::Run((scheduler.system_names[index], err))
                        })
                    })?;
                }
            }

            Ok(())
        }
        #[cfg(not(feature = "parallel"))]
        {
            batches.sequential.iter().try_for_each(|&index| {
                (scheduler.systems[index])(self)
                    .map_err(|err| error::RunWorkload::Run((scheduler.system_names[index], err)))
            })
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
    pub fn run_default(&self) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;

        if !scheduler.is_empty() {
            self.run_workload_index(&scheduler, scheduler.default_workload())?
        }
        Ok(())
    }
    /// Returns a `Ref<&AllStorages>`, used to implement custom storages.   
    /// To borrow `AllStorages` you should use `borrow` or `run` with `AllStoragesViewMut`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed.
    pub fn all_storages(&self) -> Result<Ref<'_, &'_ AllStorages>, error::Borrow> {
        self.all_storages.try_borrow()
    }
    /// Returns a `RefMut<&mut AllStorages>`, used to implement custom storages.   
    /// To borrow `AllStorages` you should use `borrow` or `run` with `AllStoragesViewMut`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed.
    pub fn all_storages_mut(&self) -> Result<RefMut<'_, &'_ mut AllStorages>, error::Borrow> {
        self.all_storages.try_borrow_mut()
    }
    /// Inserts a custom storage to the `World`.
    ///
    /// ### Errors
    ///
    /// - `AllStorages` is already borrowed exclusively.
    pub fn add_custom_storage<S: 'static + UnknownStorage + Send + Sync>(
        &self,
        storage_id: StorageId,
        storage: S,
    ) -> Result<(), error::Borrow> {
        let _ = self
            .all_storages
            .try_borrow()?
            .custom_storage_or_insert_by_id(storage_id, || storage);

        Ok(())
    }
}

impl World {
    /// Creates a new entity with the components passed as argument and returns its `EntityId`.  
    /// `component` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity0 = world.add_entity((0u32,));
    /// let entity1 = world.add_entity((1u32, 11usize));
    /// ```
    #[inline]
    pub fn add_entity<C: AddComponent>(&mut self, component: C) -> EntityId {
        self.all_storages.get_mut().add_entity(component)
    }
    /// Creates multiple new entities and returns an iterator yielding the new `EntityId`s.  
    /// `source` must always yield a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity0 = world.bulk_add_entity((0..1).map(|_| {})).next();
    /// let entity1 = world.bulk_add_entity((1..2).map(|i| (i as u32,))).next();
    /// let new_entities = world.bulk_add_entity((10..20).map(|i| (i as u32, i)));
    /// ```
    #[inline]
    pub fn bulk_add_entity<T: BulkAddEntity + 'static>(&mut self, source: T) -> BulkEntityIter<'_> {
        self.all_storages.get_mut().bulk_add_entity(source)
    }
    /// Adds components to an existing entity.  
    /// If the entity already owned a component it will be replaced.  
    /// `component` must always be a tuple, even for a single component.  
    /// Unwraps errors.
    ///
    /// ### Errors
    ///
    /// - `entity` is not alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// // make an empty entity
    /// let entity = world.add_entity(());
    ///
    /// world.add_component(entity, (0u32,));
    /// // entity already had a `u32` component so it will be replaced
    /// world.add_component(entity, (1u32, 11usize));
    /// ```
    #[track_caller]
    #[inline]
    pub fn add_component<C: AddComponent>(&mut self, entity: EntityId, component: C) {
        self.all_storages.get_mut().add_component(entity, component)
    }
    /// Removes components from an entity.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0u32, 1usize));
    ///
    /// let (i,) = world.remove::<(u32,)>(entity);
    /// assert_eq!(i, Some(0));
    /// ```
    #[inline]
    pub fn remove<C: Remove>(&mut self, entity: EntityId) -> C::Out {
        self.all_storages.get_mut().remove::<C>(entity)
    }
    /// Deletes components from an entity. As opposed to `remove`, `delete` doesn't return anything.  
    /// `C` must always be a tuple, even for a single component.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0u32, 1usize));
    ///
    /// world.delete_component::<(u32,)>(entity);
    /// ```
    #[inline]
    pub fn delete_component<C: DeleteComponent>(&mut self, entity: EntityId) {
        self.all_storages.get_mut().delete_component::<C>(entity)
    }
    /// Deletes an entity with all its components. Returns true if the entity were alive.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0u32, 1usize));
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
    /// use shipyard::World;
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0u32, 1usize));
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
    /// use shipyard::{SparseSet, World};
    ///
    /// let mut world = World::new();
    ///
    /// let entity0 = world.add_entity((0u32,));
    /// let entity1 = world.add_entity((1usize,));
    /// let entity2 = world.add_entity(("2",));
    ///
    /// // deletes `entity2`
    /// world.delete_any::<SparseSet<&str>>();
    /// // deletes `entity0` and `entity1`
    /// world.delete_any::<(SparseSet<u32>, SparseSet<usize>)>();
    /// ```
    #[inline]
    pub fn delete_any<S: DeleteAny>(&mut self) {
        self.all_storages.get_mut().delete_any::<S>();
    }
    /// Deletes all components of an entity except the ones passed in `S`.  
    /// The storage's type has to be used and not the component.  
    /// `SparseSet` is the default storage.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{SparseSet, World};
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0u32, 1usize));
    ///
    /// world.retain::<SparseSet<u32>>(entity);
    /// ```
    #[inline]
    pub fn retain<S: Retain>(&mut self, entity: EntityId) {
        self.all_storages.get_mut().retain::<S>(entity);
    }
    /// Same as `retain` but uses `StorageId` and not generics.  
    /// You should only use this method if you use a custom storage with a runtime id.
    #[inline]
    pub fn retain_storage(&mut self, entity: EntityId, excluded_storage: &[StorageId]) {
        self.all_storages
            .get_mut()
            .retain_storage(entity, excluded_storage);
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
    #[inline]
    pub fn spawn(&mut self, entity: EntityId) -> bool {
        self.all_storages.get_mut().spawn(entity)
    }
}
