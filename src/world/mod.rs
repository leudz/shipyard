mod scheduler;

pub use scheduler::WorkloadBuilder;

use crate::atomic_refcell::AtomicRefCell;
use crate::error;
//use crate::run::{Dispatch, Run, System, SystemData};
use crate::borrow::Borrow;
use crate::storage::AllStorages;
use alloc::borrow::Cow;
use core::ops::Range;
#[cfg(feature = "parallel")]
use rayon::{ThreadPool, ThreadPoolBuilder};
use scheduler::Scheduler;

/// Holds all components and keeps track of entities and what they own.
pub struct World {
    pub(crate) all_storages: AtomicRefCell<AllStorages>,
    #[cfg(feature = "parallel")]
    pub(crate) thread_pool: ThreadPool,
    scheduler: AtomicRefCell<Scheduler>,
}

impl Default for World {
    /// Create an empty `World`.
    fn default() -> Self {
        #[cfg(feature = "std")]
        {
            World {
                all_storages: AtomicRefCell::new(Default::default(), None, true),
                #[cfg(feature = "parallel")]
                thread_pool: ThreadPoolBuilder::new()
                    .num_threads(num_cpus::get_physical())
                    .build()
                    .unwrap(),
                scheduler: AtomicRefCell::new(Default::default(), None, true),
            }
        }
        #[cfg(not(feature = "std"))]
        {
            World {
                all_storages: AtomicRefCell::new(Default::default()),
                #[cfg(feature = "parallel")]
                thread_pool: ThreadPoolBuilder::new()
                    .num_threads(num_cpus::get_physical())
                    .build()
                    .unwrap(),
                scheduler: AtomicRefCell::new(Default::default()),
            }
        }
    }
}

impl World {
    /// Create an empty `World`.
    pub fn new() -> Self {
        Default::default()
    }
    /// Returns a new `World` with custom threads.  
    /// Custom threads can be useful when working with wasm for example.
    #[cfg(feature = "parallel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
    pub fn new_with_custom_threads<F: FnMut(rayon::ThreadBuilder) -> Result<(), std::io::Error>>(
        f: F,
    ) -> Self {
        World {
            all_storages: AtomicRefCell::new(Default::default(), None, true),
            #[cfg(feature = "parallel")]
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_cpus::get_physical())
                .spawn_handler(f)
                .build()
                .unwrap(),
            scheduler: AtomicRefCell::new(Default::default(), None, true),
        }
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// [Unique]: struct.Unique.html
    pub fn add_unique<T: 'static + Send + Sync>(&self, component: T) {
        self.try_add_unique(component).unwrap();
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique].  
    /// Does nothing if the storage already exists.
    ///
    /// [Unique]: struct.Unique.html
    pub fn try_add_unique<T: 'static + Send + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow_mut()?
            .register_unique(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSend].  
    /// Does nothing if the storage already exists.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSend]: struct.NonSend.html
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    pub fn try_add_unique_non_send<T: 'static + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow_mut()?
            .register_unique_non_send(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSend].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSend]: struct.NonSend.html
    #[cfg(feature = "non_send")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_send")))]
    pub fn add_unique_non_send<T: 'static + Sync>(&self, component: T) {
        self.try_add_unique_non_send::<T>(component).unwrap()
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSync].  
    /// Does nothing if the storage already exists.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSync]: struct.NonSync.html
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    pub fn try_add_unique_non_sync<T: 'static + Send>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow_mut()?
            .register_unique_non_sync(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSync].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSync]: struct.NonSync.html
    #[cfg(feature = "non_sync")]
    #[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
    pub fn add_unique_non_sync<T: 'static + Send>(&self, component: T) {
        self.try_add_unique_non_sync::<T>(component).unwrap()
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSendSync].  
    /// Does nothing if the storage already exists.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSendSync]: struct.NonSendSync.html
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    pub fn try_add_unique_non_send_sync<T: 'static>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.all_storages
            .try_borrow_mut()?
            .register_unique_non_send_sync(component);
        Ok(())
    }
    /// Adds a new unique storage, unique storages store exactly one `T` at any time.  
    /// To access a unique storage value, use [Unique] and [NonSendSync].  
    /// Does nothing if the storage already exists.  
    /// Unwraps errors.
    ///
    /// [Unique]: struct.Unique.html
    /// [NonSendSync]: struct.NonSendSync.html
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "non_send", feature = "non_sync"))))]
    pub fn add_unique_non_send_sync<T: 'static>(&self, component: T) {
        self.try_add_unique_non_send_sync::<T>(component).unwrap()
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.

You can use:
* `&T` for a shared access to `T` storage
* `&mut T` for an exclusive access to `T` storage
* [Entities] for a shared access to the entity storage
* [EntitiesMut] for an exclusive reference to the entity storage
* [AllStorages] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
* [Unique]<&T> for a shared access to a `T` unique storage
* [Unique]<&mut T> for an exclusive access to a `T` unique storage"]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"parallel\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "    * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        all(feature = "parallel", not(docsrs)),
        doc = "* [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        not(feature = "parallel"),
        doc = "* ThreadPool: must activate the *parallel* feature"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
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
        doc = "    * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
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
        doc = "    * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "### Example
```
# use shipyard::*;
let world = World::new();
let u32s = world.borrow::<&u32>();
let (entities, mut usizes) = world.try_borrow::<(Entities, &mut usize)>().unwrap();
```
[Entities]: struct.Entities.html
[EntitiesMut]: struct.Entities.html
[AllStorages]: struct.AllStorages.html
[World]: struct.World.html
[Unique]: struct.Unique.html"]
    #[cfg_attr(feature = "parallel", doc = "[ThreadPool]: struct.ThreadPool.html")]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_borrow<'s, V: Borrow<'s>>(&'s self) -> Result<V, error::GetStorage> {
        #[cfg(feature = "parallel")]
        {
            V::try_borrow(&self.all_storages, &self.thread_pool)
        }
        #[cfg(not(feature = "parallel"))]
        {
            V::try_borrow(&self.all_storages)
        }
    }
    #[doc = "Borrows the requested storage(s), if it doesn't exist it'll get created.  
You can use a tuple to get multiple storages at once.  
Unwraps errors.

You can use:
* `&T` for a shared access to `T` storage
* `&mut T` for an exclusive access to `T` storage
* [Entities] for a shared access to the entity storage
* [EntitiesMut] for an exclusive reference to the entity storage
* [AllStorages] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
* [Unique]<&T> for a shared access to a `T` unique storage
* [Unique]<&mut T> for an exclusive access to a `T` unique storage"]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"parallel\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "    * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        all(feature = "parallel", not(docsrs)),
        doc = "* [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        not(feature = "parallel"),
        doc = "* ThreadPool: must activate the *parallel* feature"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
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
        doc = "    * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
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
        doc = "    * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "### Example
```
# use shipyard::*;
let world = World::new();
let u32s = world.borrow::<&u32>();
let (entities, mut usizes) = world.borrow::<(Entities, &mut usize)>();
```
[Entities]: struct.Entities.html
[EntitiesMut]: struct.Entities.html
[AllStorages]: struct.AllStorages.html
[World]: struct.World.html
[Unique]: struct.Unique.html"]
    #[cfg_attr(feature = "parallel", doc = "[ThreadPool]: struct.ThreadPool.html")]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn borrow<'s, V: Borrow<'s>>(&'s self) -> V {
        self.try_borrow::<V>().unwrap()
    }
    #[doc = "Borrows the requested storages and runs `f`, this is an unnamed system.  
You can use a tuple to get multiple storages at once.

You can use:
* `&T` for a shared access to `T` storage
* `&mut T` for an exclusive access to `T` storage
* [Entities] for a shared access to the entity storage
* [EntitiesMut] for an exclusive reference to the entity storage
* [AllStorages] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
* [Unique]<&T> for a shared access to a `T` unique storage
* [Unique]<&mut T> for an exclusive access to a `T` unique storage"]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"parallel\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "    * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        all(feature = "parallel", not(docsrs)),
        doc = "* [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        not(feature = "parallel"),
        doc = "* ThreadPool: must activate the *parallel* feature"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
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
        doc = "    * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
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
        doc = "    * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "### Example
```
# use shipyard::*;
let world = World::new();
world.try_run::<(&usize, &mut u32), _, _>(|(usizes, u32s)| {
    // -- snip --
}).unwrap();
```
[Entities]: struct.Entities.html
[EntitiesMut]: struct.Entities.html
[AllStorages]: struct.AllStorages.html
[World]: struct.World.html
[Unique]: struct.Unique.html"]
    #[cfg_attr(feature = "parallel", doc = "[ThreadPool]: struct.ThreadPool.html")]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn try_run<'s, B, R, S: crate::system::System<'s, B, R>>(
        &'s self,
        s: S,
    ) -> Result<R, error::Run> {
        Ok(s.run({
            #[cfg(feature = "parallel")]
            {
                S::try_borrow(&self.all_storages, &self.thread_pool)?
            }
            #[cfg(not(feature = "parallel"))]
            {
                S::try_borrow(&self.all_storages)?
            }
        }))
    }
    #[doc = "Borrows the requested storages and runs `f`, this is an unnamed system.  
You can use a tuple to get multiple storages at once.  
Unwraps errors.

You can use:
* `&T` for a shared access to `T` storage
* `&mut T` for an exclusive access to `T` storage
* [Entities] for a shared access to the entity storage
* [EntitiesMut] for an exclusive reference to the entity storage
* [AllStorages] for an exclusive access to the storage of all components, ⚠️ can't coexist with any other storage borrow
* [Unique]<&T> for a shared access to a `T` unique storage
* [Unique]<&mut T> for an exclusive access to a `T` unique storage"]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"parallel\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "parallel", docsrs),
        doc = "    * [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        all(feature = "parallel", not(docsrs)),
        doc = "* [ThreadPool] for a shared access to the `ThreadPool` used by the [World]"
    )]
    #[cfg_attr(
        not(feature = "parallel"),
        doc = "* ThreadPool: must activate the *parallel* feature"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "* <span style=\"display: table;color: #2f2f2f;background-color: #C4ECFF;border-width: 1px;border-style: solid;border-color: #7BA5DB;padding: 3px;margin-bottom: 5px; font-size: 90%\">This is supported on <strong><code style=\"background-color: #C4ECFF\">feature=\"non_send\"</code></strong> only:</span>"
    )]
    #[cfg_attr(
        all(feature = "non_send", docsrs),
        doc = "    * [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
    * [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", not(docsrs)),
        doc = "* [NonSend]<&T> for a shared access to a `T` storage where `T` isn't `Send`
* [NonSend]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send`  
[Unique] and [NonSend] can be used together to access a `!Send` unique storage."
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
        doc = "    * [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
    * [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_sync", not(docsrs)),
        doc = "* [NonSync]<&T> for a shared access to a `T` storage where `T` isn't `Sync`
* [NonSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Sync`  
[Unique] and [NonSync] can be used together to access a `!Sync` unique storage."
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
        doc = "    * [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
    * [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync", not(docsrs)),
        doc = "* [NonSendSync]<&T> for a shared access to a `T` storage where `T` isn't `Send` nor `Sync`
* [NonSendSync]<&mut T> for an exclusive access to a `T` storage where `T` isn't `Send` nor `Sync`  
[Unique] and [NonSendSync] can be used together to access a `!Send + !Sync` unique storage."
    )]
    #[cfg_attr(
        not(all(feature = "non_send", feature = "non_sync")),
        doc = "* NonSendSync: must activate the *non_send* and *non_sync* features"
    )]
    #[doc = "### Example
```
# use shipyard::*;
let world = World::new();
world.run::<(&usize, &mut u32), _, _>(|(usizes, u32s)| {
    // -- snip --
});
```
[Entities]: struct.Entities.html
[EntitiesMut]: struct.Entities.html
[AllStorages]: struct.AllStorages.html
[World]: struct.World.html
[Unique]: struct.Unique.html"]
    #[cfg_attr(feature = "parallel", doc = "[ThreadPool]: struct.ThreadPool.html")]
    #[cfg_attr(feature = "non_send", doc = "[NonSend]: struct.NonSend.html")]
    #[cfg_attr(feature = "non_sync", doc = "[NonSync]: struct.NonSync.html")]
    #[cfg_attr(
        all(feature = "non_send", feature = "non_sync"),
        doc = "[NonSendSync]: struct.NonSendSync.html"
    )]
    pub fn run<'s, B, R, S: crate::system::System<'s, B, R>>(&'s self, s: S) -> R {
        //pub fn run<'s, V: Borrow<'s>, R, F: FnOnce(V) -> R>(&'s self, f: F) -> R {
        self.try_run(s).unwrap()
    }
    /// Modifies the current default workload to `name`.
    pub fn try_set_default_workload(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> Result<(), error::SetDefaultWorkload> {
        let mut scheduler = self.scheduler.try_borrow_mut()?;
        if let Some(workload) = scheduler.workloads.get(&name.into()) {
            scheduler.default = workload.clone();
            Ok(())
        } else {
            Err(error::SetDefaultWorkload::MissingWorkload)
        }
    }
    /// Modifies the current default workload to `name`.  
    /// Unwraps errors.
    pub fn set_default_workload(&self, name: impl Into<Cow<'static, str>>) {
        self.try_set_default_workload(name).unwrap();
    }
    /// A workload is a collection of systems. They will execute as much in parallel as possible.  
    /// They are evaluated left to right when they can't be parallelized.  
    /// The default workload will automatically be set to the first workload added.
    ///
    /// ### Example
    /// ```
    /// # use shipyard::*;
    /// struct Adder;
    /// impl<'a> System<'a> for Adder {
    ///     type Data = (&'a mut usize, &'a u32);
    ///     fn run((mut usizes, u32s): <Self::Data as SystemData>::View) {
    ///         (&mut usizes, &u32s).iter().for_each(|(x, &y)| {
    ///             *x += y as usize;
    ///         });
    ///     }
    /// }
    ///
    /// struct Checker;
    /// impl<'a> System<'a> for Checker {
    ///     type Data = &'a usize;
    ///     fn run(usizes: <Self::Data as SystemData>::View) {
    ///         let mut iter = usizes.iter();
    ///         assert_eq!(iter.next(), Some(&1));
    ///         assert_eq!(iter.next(), Some(&5));
    ///         assert_eq!(iter.next(), Some(&9));
    ///     }
    /// }
    ///
    /// let world = World::new();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///     entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    /// });
    ///
    /// world.try_add_workload::<(Adder, Checker), _>("Add & Check").unwrap();
    /// world.run_default();
    /// ```
    pub fn try_add_workload(
        &self,
        name: impl Into<Cow<'static, str>>,
    ) -> Result<WorkloadBuilder<'_>, error::Borrow> {
        Ok(WorkloadBuilder::new(
            self.scheduler.try_borrow_mut()?,
            name.into(),
        ))
    }
    /// A workload is a collection of systems. They will execute as much in parallel as possible.  
    /// They are evaluated left to right when they can't be parallelized.  
    /// The default workload will automatically be set to the first workload added.  
    /// Unwraps errors.
    ///
    /// ### Example
    /// ```
    /// # use shipyard::*;
    /// struct Adder;
    /// impl<'a> System<'a> for Adder {
    ///     type Data = (&'a mut usize, &'a u32);
    ///     fn run((mut usizes, u32s): <Self::Data as SystemData>::View) {
    ///         (&mut usizes, &u32s).iter().for_each(|(x, &y)| {
    ///             *x += y as usize;
    ///         });
    ///     }
    /// }
    ///
    /// struct Checker;
    /// impl<'a> System<'a> for Checker {
    ///     type Data = &'a usize;
    ///     fn run(usizes: <Self::Data as SystemData>::View) {
    ///         let mut iter = usizes.iter();
    ///         assert_eq!(iter.next(), Some(&1));
    ///         assert_eq!(iter.next(), Some(&5));
    ///         assert_eq!(iter.next(), Some(&9));
    ///     }
    /// }
    ///
    /// let world = World::new();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///     entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    /// });
    ///
    /// world.add_workload::<(Adder, Checker), _>("Add & Check");
    /// world.run_default();
    /// ```
    pub fn add_workload(&self, name: impl Into<Cow<'static, str>>) -> WorkloadBuilder<'_> {
        self.try_add_workload(name).unwrap()
    }
    /// Runs the `name` workload.
    pub fn try_run_workload(&self, name: impl AsRef<str> + Sync) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;
        if let Some(range) = scheduler.workloads.get(name.as_ref()) {
            self.try_run_workload_index(&*scheduler, range.clone())
        } else {
            Err(error::RunWorkload::MissingWorkload)
        }
    }
    /// Runs the `name` workload.  
    /// Unwraps error.
    pub fn run_workload(&self, name: impl AsRef<str> + Sync) {
        self.try_run_workload(name).unwrap();
    }
    fn try_run_workload_index(
        &self,
        scheduler: &Scheduler,
        workload: Range<usize>,
    ) -> Result<(), error::RunWorkload> {
        for batch in &scheduler.batch[workload] {
            if batch.len() == 1 {
                scheduler.systems[batch[0]](self).map_err(|err| {
                    error::RunWorkload::Run((scheduler.system_names[batch[0]], err))
                })?;
            } else {
                #[cfg(feature = "parallel")]
                {
                    use rayon::prelude::*;

                    self.thread_pool.install(|| {
                        batch.into_par_iter().try_for_each(|&index| {
                            (scheduler.systems[index])(self).map_err(|err| {
                                error::RunWorkload::Run((scheduler.system_names[index], err))
                            })
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
    /// Run the default workload.
    pub fn try_run_default(&self) -> Result<(), error::RunWorkload> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| error::RunWorkload::Scheduler)?;
        if !scheduler.batch.is_empty() {
            self.try_run_workload_index(&scheduler, scheduler.default.clone())?
        }
        Ok(())
    }
    /// Run the default workload.  
    /// Unwraps error.
    pub fn run_default(&self) {
        self.try_run_default().unwrap();
    }
}
