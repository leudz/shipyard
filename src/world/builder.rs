use crate::all_storages::{AllStoragesBuilder, LockPresent, ThreadIdPresent, TupleGroup};
use crate::atomic_refcell::AtomicRefCell;
use crate::public_transport::ShipyardRwLock;
use crate::world::World;
use alloc::sync::Arc;
use core::sync::atomic::AtomicU64;

/// Builder for [`World`] to declare [groups](WorldBuilder::with_group)
/// or configure a custom lock, thread pool or thread id provider function.
pub struct WorldBuilder<Lock, ThreadId> {
    all_storages_builder: AllStoragesBuilder<Lock, ThreadId>,
    #[cfg(feature = "parallel")]
    thread_pool: Option<rayon::ThreadPool>,
}

impl World {
    /// Returns a builder for [`World`] to declare [groups](WorldBuilder::with_group)
    /// or configure a custom lock, thread pool or thread id provider function.
    #[cfg(feature = "std")]
    pub fn builder() -> WorldBuilder<LockPresent, ThreadIdPresent> {
        WorldBuilder {
            all_storages_builder: AllStoragesBuilder::<LockPresent, ThreadIdPresent>::new(),
            #[cfg(feature = "parallel")]
            thread_pool: None,
        }
    }

    /// Returns a builder for [`World`] to declare [groups](WorldBuilder::with_group)
    /// or configure a custom lock, thread pool or thread id provider function.
    #[cfg(all(not(feature = "std"), not(feature = "thread_local")))]
    pub fn builder() -> WorldBuilder<crate::all_storages::MissingLock, ThreadIdPresent> {
        WorldBuilder {
            all_storages_builder: AllStoragesBuilder::<
                crate::all_storages::MissingLock,
                ThreadIdPresent,
            >::new(),
            #[cfg(feature = "parallel")]
            thread_pool: None,
        }
    }

    /// Returns a builder for [`World`] to declare [groups](WorldBuilder::with_group)
    /// or configure a custom lock, thread pool or thread id provider function.
    #[cfg(all(not(feature = "std"), feature = "thread_local"))]
    pub fn builder()
    -> WorldBuilder<crate::all_storages::MissingLock, crate::all_storages::MissingThreadId> {
        WorldBuilder {
            all_storages_builder: AllStoragesBuilder::<
                crate::all_storages::MissingLock,
                crate::all_storages::MissingThreadId,
            >::new(),
            #[cfg(feature = "parallel")]
            thread_pool: None,
        }
    }
}

impl<Lock, ThreadId> WorldBuilder<Lock, ThreadId> {
    /// Use a custom `RwLock` for [`AllStorages`].
    ///
    /// [`AllStorages`]: crate::all_storages::AllStorages
    pub fn with_custom_lock<L: ShipyardRwLock + Send + Sync>(
        self,
    ) -> WorldBuilder<LockPresent, ThreadId> {
        WorldBuilder {
            all_storages_builder: self.all_storages_builder.with_custom_lock::<L>(),
            #[cfg(feature = "parallel")]
            thread_pool: self.thread_pool,
        }
    }

    /// Use a custom function to provide the current thread id.
    ///
    /// If the target platform doesn't have threads you can return a random integer.
    ///
    /// ```
    /// use shipyard::World;
    ///
    /// let world = World::builder().with_custom_thread_id(|| 0).build();
    /// ```
    #[cfg(feature = "thread_local")]
    pub fn with_custom_thread_id(
        self,
        thread_id: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> WorldBuilder<Lock, ThreadIdPresent> {
        WorldBuilder {
            all_storages_builder: self.all_storages_builder.with_custom_thread_id(thread_id),
            #[cfg(feature = "parallel")]
            thread_pool: self.thread_pool,
        }
    }

    /// Use a local [`ThreadPool`](rayon::ThreadPool).
    ///
    /// This is useful when you have multiple [`Worlds`](World) or something else using [`rayon`] and want them to stay isolated.\
    /// For example with a single [`ThreadPool`](rayon::ThreadPool), a panic would take down all [`Worlds`](World).\
    /// With a [`ThreadPool`](rayon::ThreadPool) per [`World`] we can keep the panic confined to a single [`World`].
    #[cfg(feature = "parallel")]
    pub fn with_local_thread_pool(
        mut self,
        thread_pool: rayon::ThreadPool,
    ) -> WorldBuilder<Lock, ThreadId> {
        self.thread_pool = Some(thread_pool);

        self
    }

    /// Declares a group made of the given storages.
    ///
    /// Iterating a group's storages together walks their components without check.\
    /// Other storages in the query are looked up as usual.
    ///
    /// [`build`](WorldBuilder::build) creates the storages that don't exist yet.\
    /// Declaring the same group twice, in any order, declares it once.
    ///
    /// ### Overlapping groups
    ///
    /// A storage can belong to multiple groups. When groups overlap, a union is created.\
    /// With `A+B` and `A+C` groups, an entity owning A, B and C moves to an `A+B+C` union.\
    /// An `(&a, &b)` query then walks both `A+B` and `A+B+C`.
    ///
    /// The more groups overlap, the more pieces a group is split into and the slower iteration gets.
    ///
    /// ### Connected sets
    ///
    /// Groups sharing a storage, directly or through other groups, form a connected set.\
    /// With `A+B`, `B+C` and `D+E` groups, `A`, `B` and `C` form one connected set and `D` and `E` another.\
    /// A set can only be regrouped as a whole, see [`World::regroup_set`].
    ///
    /// Groups can only be declared here, so the connected sets are fixed once the world is built.
    ///
    /// ### Regrouping
    ///
    /// Components added to a group's storages wait outside the group until a regroup,
    /// removals can also leave the group out of place.\
    /// Iteration stays correct meanwhile, it's only slower.
    ///
    /// - [`World::regroup`] and [`AllStorages::regroup`] regroups the whole world.
    /// - [`World::regroup_set`] and [`AllStorages::regroup_set`] regroups a single connected set.
    /// - Workloads added to the world regroup what they use automatically.
    ///
    /// ### Workloads
    ///
    /// Workloads added with [`Workload::add_to_world`] or [`World::add_workload`] regroup the
    /// connected sets their systems borrow:
    ///
    /// - Each set is regrouped before the first system borrowing one of its storages, to take in
    ///   the changes made outside the workload. Workloads that only read get this regroup too.
    /// - A system borrowing a grouped storage exclusively, with a `ViewMut` for example, leaves it
    ///   to regroup once it ran. Even if it only changed values, the workload can't know.\
    ///   A system skipped by its condition doesn't.
    /// - The set of that storage is then regrouped before the storage's next borrow,
    ///   and before the workload ends.
    ///
    /// A regroup is only needed before the written storage is used again. Other storages of its set can be used in the meantime.\
    /// With an `A+B` group, `write A -> read B -> read A` runs as
    /// `write A -> read B -> regroup A+B -> read A`. Reading `B` doesn't wait for the regroup,
    /// reading `A` does. A single regroup takes care of every storage of the set written since the last one.
    ///
    /// A regroup borrows every storage of its set exclusively, including the ones the systems don't use.\
    /// If one of them is already borrowed, the workload returns [`RunWorkload::Regroup`].
    ///
    /// When a system returns an error, the workload stops without regrouping what was left to regroup.\
    /// The next workload using these sets regroups them when it starts.
    ///
    /// Only the storages systems borrow directly count. Borrows made by `run_if` or `skip_if` conditions,
    /// or through `AllStorages`, don't wait for a regroup, and their changes don't make the workload regroup.\
    /// Use [`AllStorages::regroup_set`] when they need one.
    ///
    /// Workloads made with [`Workload::build`] don't regroup, they aren't tied to a world.
    ///
    /// ### Current limitations
    ///
    /// - All of a group's storages have to be in the same tuple when creating an iterator.\
    ///   `((&a, &b), &c)` uses an `A+B` group, `(&a, (&b, &c))` doesn't.
    /// - Queries using `Not`, `Optional`, `Or` or a tracking filter like `inserted` don't use groups.
    /// - Only bulk consumers like `for_each` or `fold` benefit from groups.\
    ///   A `for` loop, or any other use of `next`, can be slower than without groups.
    ///
    /// ### Panics
    ///
    /// - The same storage is listed twice.
    ///
    /// ### Example
    ///
    /// ```
    /// use shipyard::{Component, IntoIter, View, ViewMut, World};
    ///
    /// #[derive(Component)]
    /// struct Pos(f32);
    ///
    /// #[derive(Component)]
    /// struct Vel(f32);
    ///
    /// fn movement(mut positions: ViewMut<Pos>, velocities: View<Vel>) {
    ///     (&mut positions, &velocities)
    ///         .iter()
    ///         .for_each(|(pos, vel)| pos.0 += vel.0);
    /// }
    ///
    /// let mut world = World::builder().with_group::<(Pos, Vel)>().build();
    /// world.add_entity((Pos(0.0), Vel(1.0)));
    ///
    /// // The workload regroups `Pos` and `Vel` before `movement` runs.
    /// world.add_workload(|| movement);
    /// world.run_default_workload().unwrap();
    ///
    /// // Outside of workloads, we regroup ourselves.
    /// world.add_entity((Pos(1.0), Vel(1.0)));
    /// world.regroup();
    /// ```
    ///
    /// [`Workload::add_to_world`]: crate::Workload::add_to_world
    /// [`Workload::build`]: crate::Workload::build
    /// [`RunWorkload::Regroup`]: crate::error::RunWorkload::Regroup
    /// [`AllStorages::regroup`]: crate::all_storages::AllStorages::regroup
    /// [`AllStorages::regroup_set`]: crate::all_storages::AllStorages::regroup_set
    #[track_caller]
    pub fn with_group<G: TupleGroup>(mut self) -> WorldBuilder<Lock, ThreadId> {
        self.all_storages_builder.grouping.declare::<G>();

        self
    }
}

impl WorldBuilder<LockPresent, ThreadIdPresent> {
    /// Creates a new [`World`] based on the [`WorldBuilder`] config.
    pub fn build(self) -> World {
        let counter = Arc::new(AtomicU64::new(1));

        let all_storages = self.all_storages_builder.build(counter.clone());

        World {
            all_storages,
            scheduler: AtomicRefCell::new(Default::default()),
            counter,
            #[cfg(feature = "parallel")]
            thread_pool: self.thread_pool,
        }
    }
}
