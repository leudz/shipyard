mod pack;
mod pipeline;
mod register;

use crate::atomic_refcell::AtomicRefCell;
use crate::component_storage::AllStorages;
use crate::error;
use crate::run::Run;
use crate::sparse_set::{Pack, UpdatePack};
use pack::{LoosePack, TightPack};
use pipeline::{Pipeline, Workload};
#[cfg(feature = "parallel")]
use rayon::{ThreadPool, ThreadPoolBuilder};
use register::Register;
use std::marker::PhantomData;

/// Holds all components and keeps track of entities and what they own.
pub struct World {
    pub(crate) storages: AtomicRefCell<AllStorages>,
    #[cfg(feature = "parallel")]
    pub(crate) thread_pool: ThreadPool,
    pipeline: AtomicRefCell<Pipeline>,
    _not_send: PhantomData<*const ()>,
}

// World can't be Send because it can contain !Send types which shouldn't be dropped on a different thread
unsafe impl Sync for World {}

impl Default for World {
    /// Create an empty `World` without any storage.
    fn default() -> Self {
        World {
            storages: AtomicRefCell::new(Default::default()),
            #[cfg(feature = "parallel")]
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_cpus::get_physical())
                .build()
                .unwrap(),
            pipeline: AtomicRefCell::new(Default::default()),
            _not_send: PhantomData,
        }
    }
}

impl World {
    /// Returns a new `World` with storages based on `T` already created.
    /// More storages can be added latter.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// `World` is never used mutably.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize,)>();
    /// let world = World::new::<(usize, u32)>();
    /// ```
    pub fn new<T: Register>() -> Self {
        let world = World::default();
        T::register(&world);
        world
    }
    /// Returns a new `World` with storages based on `T` already created
    /// and custom threads.
    /// More storages can be added latter.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// Custom threads can be useful when working with wasm for example.
    ///
    /// `World` is never used mutably.
    #[cfg(feature = "parallel")]
    pub fn new_with_custom_threads<
        T: Register,
        F: FnMut(rayon::ThreadBuilder) -> Result<(), std::io::Error>,
    >(
        f: F,
    ) -> Self {
        World {
            storages: AtomicRefCell::new(Default::default()),
            thread_pool: ThreadPoolBuilder::new()
                .num_threads(num_cpus::get_physical())
                .spawn_handler(f)
                .build()
                .unwrap(),
            pipeline: AtomicRefCell::new(Default::default()),
            _not_send: PhantomData,
        }
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
    /// Register a new component type and create a unique storage for it.
    /// Does nothing if the storage already exists.
    ///
    /// Unique storages store exactly one `T` at any time.
    /// To access a unique storage value, use [Unique].
    ///
    /// Unwraps errors.
    ///
    /// [Unique]: struct.Unique.html
    pub fn register_unique<T: 'static + Send + Sync>(&self, component: T) {
        self.try_register_unique(component).unwrap();
    }
    /// Register a new component type and create a unique storage for it.
    /// Does nothing if the storage already exists.
    ///
    /// Unique storages store exactly one `T` at any time.
    /// To access a unique storage value, use [Unique].
    ///
    /// [Unique]: struct.Unique.html
    pub fn try_register_unique<T: 'static + Send + Sync>(
        &self,
        component: T,
    ) -> Result<(), error::Borrow> {
        self.storages.try_borrow_mut()?.register_unique(component);
        Ok(())
    }
    /// Allows to perform some actions not possible otherwise like iteration.
    /// This is basically an unnamed system.
    ///
    /// `T` can be:
    /// * `&T` for an immutable reference to `T` storage
    /// * `&mut T` for a mutable reference to `T` storage
    /// * [Entities] for an immutable reference to the entity storage
    /// * [EntitiesMut] for a mutable reference to the entity storage
    /// * [AllStorages] for a mutable reference to the storage of all components
    /// * [ThreadPool] for an immutable reference to the `rayon::ThreadPool` used by the [World]
    /// * [Not] can be used to filter out a component type
    ///
    /// A tuple will allow multiple references.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
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
            T::run(&self.storages, &self.thread_pool, f);
        }
        #[cfg(not(feature = "parallel"))]
        {
            T::run(&self.storages, f);
        }
    }
    /// Pack multiple storages together, it can speed up iteration at a small cost on insertion/removal.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.try_tight_pack::<(usize, u32)>().unwrap();
    /// ```
    pub fn try_tight_pack<T: TightPack>(&self) -> Result<(), error::Pack> {
        T::try_tight_pack(&self.storages)
    }
    /// Pack multiple storages together, it can speed up iteration at a small cost on insertion/removal.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.tight_pack::<(usize, u32)>();
    /// ```
    pub fn tight_pack<T: TightPack>(&self) {
        self.try_tight_pack::<T>().unwrap()
    }
    pub fn try_loose_pack<T, L>(&self) -> Result<(), error::Pack>
    where
        (T, L): LoosePack,
    {
        <(T, L)>::try_loose_pack(&self.storages)
    }
    pub fn loose_pack<T, L>(&self)
    where
        (T, L): LoosePack,
    {
        <(T, L)>::try_loose_pack(&self.storages).unwrap()
    }
    pub fn try_update_pack<T: 'static>(&self) -> Result<(), error::Pack> {
        let all_storages = self
            .storages
            .try_borrow()
            .map_err(error::GetStorage::AllStoragesBorrow)?;
        if let Some(storage) = all_storages.0.get(&std::any::TypeId::of::<T>()) {
            let mut sparse_set = storage
                .sparse_set_mut::<T>()
                .map_err(error::GetStorage::StorageBorrow)?;
            if sparse_set.is_unique() {
                return Err(error::Pack::UniqueStorage(std::any::type_name::<T>()));
            }
            match sparse_set.pack_info.pack {
                Pack::NoPack => {
                    sparse_set.pack_info.pack = Pack::Update(UpdatePack {
                        inserted: sparse_set.len(),
                        modified: 0,
                        removed: Vec::new(),
                    });
                    Ok(())
                }
                Pack::Tight(_) => Err(error::Pack::AlreadyTightPack(std::any::TypeId::of::<T>())),
                Pack::Loose(_) => Err(error::Pack::AlreadyLoosePack(std::any::TypeId::of::<T>())),
                Pack::Update(_) => Err(error::Pack::AlreadyUpdatePack(std::any::TypeId::of::<T>())),
            }
        } else {
            Err(error::GetStorage::MissingComponent.into())
        }
    }
    pub fn update_pack<T: 'static>(&self) {
        self.try_update_pack::<T>().unwrap();
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
    /// # use shipyard::prelude::*;
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
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///     entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    /// });
    ///
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
    /// # use shipyard::prelude::*;
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
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///     entities.add_entity((&mut usizes, &mut u32s), (2, 3));
    ///     entities.add_entity((&mut usizes, &mut u32s), (4, 5));
    /// });
    ///
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
