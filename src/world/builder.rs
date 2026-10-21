use crate::all_storages::{AllStoragesBuilder, LockPresent, ThreadIdPresent, TupleGroup};
use crate::atomic_refcell::AtomicRefCell;
use crate::public_transport::ShipyardRwLock;
use crate::world::World;
use alloc::sync::Arc;
use core::sync::atomic::AtomicU64;

/// Builder for [`World`] when one wants custom lock, custom thread pool
/// or custom thread id provider function.
pub struct WorldBuilder<Lock, ThreadId> {
    all_storages_builder: AllStoragesBuilder<Lock, ThreadId>,
    #[cfg(feature = "parallel")]
    thread_pool: Option<rayon::ThreadPool>,
}

impl World {
    /// Returns a builder for [`World`] when one wants custom lock, custom thread pool
    /// or custom thread id provider function.
    #[cfg(feature = "std")]
    pub fn builder() -> WorldBuilder<LockPresent, ThreadIdPresent> {
        WorldBuilder {
            all_storages_builder: AllStoragesBuilder::<LockPresent, ThreadIdPresent>::new(),
            #[cfg(feature = "parallel")]
            thread_pool: None,
        }
    }

    /// Returns a builder for [`World`] when one wants custom lock, custom thread pool
    /// or custom thread id provider function.
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

    /// Returns a builder for [`World`] when one wants custom lock, custom thread pool
    /// or custom thread id provider function.
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
    /// use shipyard::{Component, World};
    ///
    /// #[derive(Component)]
    /// struct Pos(f32);
    ///
    /// #[derive(Component)]
    /// struct Vel(f32);
    ///
    /// let world = World::builder().with_group::<(Pos, Vel)>().build();
    /// ```
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
