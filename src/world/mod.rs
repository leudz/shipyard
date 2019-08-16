mod new_entity;
mod pack;
mod pipeline;
mod register;

use crate::atomic_refcell::{AtomicRefCell, Ref, RefMut};
use crate::component_storage::AllStorages;
use crate::entity::{Entities, Key};
use crate::error;
use crate::get_storage::GetStorage;
use crate::pack::OwnedPack;
use crate::run::Run;
use new_entity::WorldNewEntity;
use pack::WorldOwnedPack;
use pipeline::{Pipeline, Workload};
#[cfg(feature = "parallel")]
use rayon::{ThreadPool, ThreadPoolBuilder};
use register::Register;

/// Holds all components and keeps track of entities and what they own.
pub struct World {
    pub(crate) entities: AtomicRefCell<Entities>,
    pub(crate) storages: AtomicRefCell<AllStorages>,
    #[cfg(feature = "parallel")]
    pub(crate) thread_pool: ThreadPool,
    pipeline: AtomicRefCell<Pipeline>,
}

impl Default for World {
    /// Create an empty `World` without any storage.
    fn default() -> Self {
        World {
            entities: AtomicRefCell::new(Default::default()),
            storages: AtomicRefCell::new(Default::default()),
            #[cfg(feature = "parallel")]
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_cpus::get_physical())
                .build()
                .unwrap(),
            pipeline: AtomicRefCell::new(Default::default()),
        }
    }
}

impl World {
    /// Creates a `World` with storages based on `T` already created.
    /// More storages can be added latter.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// `World` is never used mutably.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize,)>();
    /// let world = World::new::<(usize, u32)>();
    /// ```
    pub fn new<T: Register>() -> Self {
        let world = World::default();
        T::register(&world);
        world
    }
    /// Retrives storages based on type `T` consuming the `RefMut<AllStorages>` in the process
    /// to only borrow it immutably.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let (usizes, u32s) = world.get_storage::<(&mut usize, &u32)>();
    /// ```
    pub fn get_storage<'a, T: GetStorage<'a>>(&'a self) -> T::Storage {
        self.try_get_storage::<T>().unwrap()
    }
    /// Retrives storages based on type `T` consuming the `RefMut<AllStorages>` in the process
    /// to only borrow it immutably.
    ///
    /// `&T` returns a read access to the storage.
    ///
    /// `&mut T` returns a write access to the storage.
    ///
    /// To retrive multiple storages at once, use a tuple.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let (usizes, u32s) = world.try_get_storage::<(&mut usize, &u32)>().unwrap();
    /// ```
    pub fn try_get_storage<'a, T: GetStorage<'a>>(
        &'a self,
    ) -> Result<T::Storage, error::GetStorage> {
        Ok(self
            .try_all_storages()
            .map_err(error::GetStorage::AllStoragesBorrow)?
            .try_get_storage::<T>()?)
    }
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    ///
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// As opposed to [Entities::add] and [EntitiesViewMut::add], storages will be created if they don't exist.
    /// This is at the cost of borrow/release `Entities` and the storages involved.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::default();
    /// let entity = world.new_entity((0usize, 1u32));
    /// ```
    /// [Entities::add]: struct.Entities.html#method.add
    /// [EntitiesViewMut::add]: struct.EntitiesViewMut.html#method.add
    pub fn new_entity<T: WorldNewEntity>(&self, component: T) -> Key {
        self.try_new_entity::<T>(component).unwrap()
    }
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    ///
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// As opposed to [Entities::add] and [EntitiesViewMut::add], storages will be created if they don't exist.
    /// This is at the cost of borrow/release `Entities` and the storages involved.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::default();
    /// let entity = world.try_new_entity((0usize, 1u32)).unwrap();
    /// ```
    /// [Entities::add]: struct.Entities.html#method.add
    /// [EntitiesViewMut::add]: struct.EntitiesViewMut.html#method.add
    pub fn try_new_entity<T: WorldNewEntity>(&self, component: T) -> Result<Key, error::NewEntity> {
        let mut entities = self
            .try_entities_mut()
            .map_err(error::NewEntity::Entities)?;
        let mut storages = self
            .storages
            .try_borrow_mut()
            .map_err(error::NewEntity::AllStoragesBorrow)?;
        Ok(T::new_entity(component, &mut *storages, &mut *entities))
    }
    /// Returns a reference to the entities' storage.
    ///
    /// Unwraps errors.
    pub fn entities(&self) -> Ref<Entities> {
        self.try_entities().unwrap()
    }
    /// Returns a reference to the entities' storage.
    pub fn try_entities(&self) -> Result<Ref<Entities>, error::Borrow> {
        Ok(self.entities.try_borrow()?)
    }
    /// Returns a mutable reference to the entities' storage.
    ///
    /// Unwraps errors.
    pub fn entities_mut(&self) -> RefMut<Entities> {
        self.try_entities_mut().unwrap()
    }
    /// Returns a mutable reference to the entities' storage.
    pub fn try_entities_mut(&self) -> Result<RefMut<Entities>, error::Borrow> {
        Ok(self.entities.try_borrow_mut()?)
    }
    /// Returns an immutable reference to the storage of all storages.
    ///
    /// Unwraps errors.
    pub fn all_storages(&self) -> Ref<AllStorages> {
        self.try_all_storages().unwrap()
    }
    /// Returns an immutable reference to the storage of all storages.
    pub fn try_all_storages(&self) -> Result<Ref<AllStorages>, error::Borrow> {
        Ok(self.storages.try_borrow()?)
    }
    /// Returns an immutable reference to the storage of all storages.
    ///
    /// Unwraps errors.
    pub fn all_storages_mut(&self) -> RefMut<AllStorages> {
        self.try_all_storages_mut().unwrap()
    }
    /// Returns an immutable reference to the storage of all storages.
    pub fn try_all_storages_mut(&self) -> Result<RefMut<AllStorages>, error::Borrow> {
        Ok(self.storages.try_borrow_mut()?)
    }
    /// Register a new component type and create a storage for it.
    /// Does nothing if the storage already exists.
    ///
    /// Unwraps errors.
    pub fn register<T: 'static + Send + Sync>(&self) {
        self.try_register::<T>().unwrap()
    }
    /// Register a new component type and create a storage for it.
    /// Does nothing if the storage already exists.
    pub fn try_register<T: 'static + Send + Sync>(&self) -> Result<(), error::Borrow> {
        self.storages.try_borrow_mut()?.register::<T>();
        Ok(())
    }
    /// Allows to perform some actions not possible otherwise like iteration.
    /// This is basically an unnamed system.
    ///
    /// `T` can be:
    /// * `&T` for an immutable reference to `T` storage
    /// * `&mut T` for a mutable reference to `T` storage
    /// * [Entities] for a mutable reference to the entity storage
    /// * [AllStorages] for a mutable reference to the storage of all components
    /// * [ThreadPool] for an immutable reference to the `rayon::ThreadPool` used by the [World]
    /// * [Not] can be used to filter out a component type
    ///
    /// A tuple will allow multiple references.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.run::<(&usize, &mut u32), _>(|(usizes, u32s)| {
    ///     // -- snip --
    /// });
    /// ```
    /// [Entities]: struct.Entities.html
    /// [AllStorages]: struct.AllStorages.html
    /// [ThreadPool]: struct.ThreadPool.html
    /// [World]: struct.World.html
    /// [Not]: struct.Not.html
    pub fn run<'a, T: Run<'a>, F: FnOnce(T::Storage)>(&'a self, f: F) {
        #[cfg(feature = "parallel")]
        {
            T::run(&self.entities, &self.storages, &self.thread_pool, f);
        }
        #[cfg(not(feature = "parallel"))]
        {
            T::run(&self.entities, &self.storages, f);
        }
    }
    /// Pack multiple storages together, it can speed up iteration at a small cost on insertion/removal.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.try_pack_owned::<(usize, u32)>().unwrap();
    /// ```
    pub fn try_pack_owned<'a, T: WorldOwnedPack<'a>>(&'a self) -> Result<(), error::WorldPack>
    where
        <T as WorldOwnedPack<'a>>::Storage: GetStorage<'a>,
        <<T as WorldOwnedPack<'a>>::Storage as GetStorage<'a>>::Storage: OwnedPack,
    {
        self.try_get_storage::<T::Storage>()?.try_pack_owned()?;
        Ok(())
    }
    /// Pack multiple storages together, it can speed up iteration at a small cost on insertion/removal.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.pack_owned::<(usize, u32)>();
    /// ```
    pub fn pack_owned<'a, T: WorldOwnedPack<'a>>(&'a self)
    where
        <T as WorldOwnedPack<'a>>::Storage: GetStorage<'a>,
        <<T as WorldOwnedPack<'a>>::Storage as GetStorage<'a>>::Storage: OwnedPack,
    {
        self.try_pack_owned::<T>().unwrap()
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    pub fn try_delete(&self, entity: Key) -> Result<bool, error::Borrow> {
        let mut entities = self.try_entities_mut()?;
        let storages = self.try_all_storages_mut()?;
        Ok(entities.delete(storages, entity))
    }
    /// Delete an entity and all its components.
    /// Returns true if the entity was alive.
    ///
    /// Unwraps errors.
    pub fn delete(&self, entity: Key) -> bool {
        self.try_delete(entity).unwrap()
    }
    /// Modifies the current default workload to `name`.
    pub fn try_set_default_workload(
        &self,
        name: impl ToString,
    ) -> Result<(), error::SetDefaultWorkload> {
        let name = name.to_string();
        let mut pipeline = self.pipeline.try_borrow_mut()?;
        if let Some(workload) = pipeline.workloads.get(&name) {
            pipeline.default = workload.clone();
            Ok(())
        } else {
            Err(error::SetDefaultWorkload::MissingWorkload)
        }
    }
    /// Modifies the current default workload to `name`.
    ///
    /// Unwraps errors.
    pub fn set_default_workload(&self, name: impl ToString) {
        self.try_set_default_workload(name).unwrap();
    }
    /// A workload is a collection of systems.
    /// They will execute as much in parallel as possible.
    ///
    /// They are evaluated left to right when they can't be parallelized.
    ///
    /// The default workload will automatically be set to the first workload added.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// struct Adder;
    /// impl<'a> System<'a> for Adder {
    ///     type Data = (&'a mut usize, &'a u32);
    ///     fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
    ///         for (x, &y) in (usizes, u32s).iter() {
    ///             *x += y as usize;
    ///         }
    ///     }
    /// }
    ///
    /// struct Checker;
    /// impl<'a> System<'a> for Checker {
    ///     type Data = &'a usize;
    ///     fn run(&self, usizes: <Self::Data as SystemData>::View) {
    ///         let mut iter = usizes.iter();
    ///         assert_eq!(iter.next(), Some(&1));
    ///         assert_eq!(iter.next(), Some(&5));
    ///         assert_eq!(iter.next(), Some(&9));
    ///     }
    /// }
    ///
    /// let world = World::new::<(usize, u32)>();
    /// world.new_entity((0usize, 1u32));
    /// world.new_entity((2usize, 3u32));
    /// world.new_entity((4usize, 5u32));
    /// world.try_add_workload("Add & Check", (Adder, Checker)).unwrap();
    /// world.run_default();
    /// ```
    pub fn try_add_workload<T: Workload>(
        &self,
        name: impl ToString,
        system: T,
    ) -> Result<(), error::Borrow> {
        let mut pipeline = self.pipeline.try_borrow_mut()?;
        system.into_workload(name.to_string(), &mut *pipeline);
        Ok(())
    }
    /// A workload is a collection of systems.
    /// They will execute as much in parallel as possible.
    ///
    /// They are evaluated left to right when they can't be parallelized.
    ///
    /// The default workload will automatically be set to the first workload added.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// struct Adder;
    /// impl<'a> System<'a> for Adder {
    ///     type Data = (&'a mut usize, &'a u32);
    ///     fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
    ///         for (x, &y) in (usizes, u32s).iter() {
    ///             *x += y as usize;
    ///         }
    ///     }
    /// }
    ///
    /// struct Checker;
    /// impl<'a> System<'a> for Checker {
    ///     type Data = &'a usize;
    ///     fn run(&self, usizes: <Self::Data as SystemData>::View) {
    ///         let mut iter = usizes.iter();
    ///         assert_eq!(iter.next(), Some(&1));
    ///         assert_eq!(iter.next(), Some(&5));
    ///         assert_eq!(iter.next(), Some(&9));
    ///     }
    /// }
    ///
    /// let world = World::new::<(usize, u32)>();
    /// world.new_entity((0usize, 1u32));
    /// world.new_entity((2usize, 3u32));
    /// world.new_entity((4usize, 5u32));
    /// world.add_workload("Add & Check", (Adder, Checker));
    /// world.run_default();
    /// ```
    pub fn add_workload<T: Workload>(&self, name: impl ToString, system: T) {
        self.try_add_workload(name, system).unwrap();
    }
    /// Runs the `name` workload.
    pub fn try_run_workload(&self, name: impl AsRef<str>) -> Result<(), error::RunWorkload> {
        let pipeline = self.pipeline.try_borrow()?;
        if let Some(workload) = pipeline.workloads.get(name.as_ref()) {
            for batch in &pipeline.batch[workload.clone()] {
                #[cfg(feature = "parallel")]
                {
                    use rayon::prelude::*;

                    self.thread_pool.install(|| {
                        batch.into_par_iter().for_each(|&index| {
                            pipeline.systems[index].dispatch(&self);
                        });
                    })
                }
                #[cfg(not(feature = "parallel"))]
                {
                    batch.iter().for_each(|&index| {
                        pipeline.systems[index].dispatch(&self);
                    });
                }
            }
            Ok(())
        } else {
            Err(error::RunWorkload::MissingWorkload)
        }
    }
    /// Runs the `name` workload.
    ///
    /// Unwraps error.
    pub fn run_workload(&self, name: impl AsRef<str>) {
        self.try_run_workload(name).unwrap();
    }
    /// Run the default workload.
    pub fn try_run_default(&self) -> Result<(), error::Borrow> {
        let pipeline = self.pipeline.try_borrow()?;
        for batch in &pipeline.batch[pipeline.default.clone()] {
            #[cfg(feature = "parallel")]
            {
                use rayon::prelude::*;

                self.thread_pool.install(|| {
                    batch.into_par_iter().for_each(|&index| {
                        pipeline.systems[index].dispatch(&self);
                    });
                })
            }
            #[cfg(not(feature = "parallel"))]
            {
                batch.iter().for_each(|&index| {
                    pipeline.systems[index].dispatch(&self);
                });
            }
        }
        Ok(())
    }
    /// Run the default workload.
    ///
    /// Unwraps error.
    pub fn run_default(&self) {
        self.try_run_default().unwrap();
    }
}
