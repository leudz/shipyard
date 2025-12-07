//! Shipyard is an Entity Component System focused on usability and speed.
//!
//! The [user guide](https://leudz.github.io/shipyard/guide/master) is a great place to learn all about Shipyard!
//!
//! ## Features
//!
//! - **parallel** *(default)* &mdash; enables workload threading and add parallel iterators
//! - **extended_tuple** &mdash; extends implementations from the default 10 to 32 tuple size at the cost of 4X build time
//! - **proc** *(default)* &mdash; re-exports macros from `shipyard_proc`, mainly to derive `Component`
//! - **serde1** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
//! - **std** *(default)* &mdash; lets Shipyard use the standard library
//! - **thread_local** &mdash; adds methods and types required to work with `!Send` and `!Sync` components
//! - **tracing** &mdash; reports workload and system execution

#![warn(elided_lifetimes_in_paths)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unused_qualifications)]
#![warn(clippy::used_underscore_binding)]
#![warn(clippy::similar_names)]
#![warn(clippy::invalid_upcast_comparisons)]
#![warn(clippy::cast_precision_loss)]
#![warn(clippy::cast_possible_wrap)]
#![warn(clippy::mutex_integer)]
#![warn(clippy::mut_mut)]
#![warn(clippy::items_after_statements)]
#![warn(clippy::print_stdout)]
#![warn(clippy::maybe_infinite_iter)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_lifetimes)]
// The question mark operator can damage performance
#![allow(clippy::question_mark)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

mod add_component;
mod add_distinct_component;
mod add_entity;
/// Contains all storages present in the [`World`].
pub mod all_storages;
/// Allows access to helper types needed to implement [`Borrow`](borrow::Borrow).
pub mod borrow;
mod component;
mod contains;
mod delete;
mod entities;
mod entity_id;
pub mod error;
mod get;
/// Contains all items related to storage iteration.
pub mod iter;
/// Module describing internal memory usage.
pub mod memory_usage;
mod r#mut;
mod not;
mod optional;
mod or;
mod public_transport;
mod remove;
/// Stores systems from all workloads and their scheduling.
pub mod scheduler;
mod seal;
/// Default component storage.
pub mod sparse_set;
mod storage;
/// Module related to storage tracking, like insertion or modification.
pub mod track;
mod unique;
mod views;
/// Contains all data this library will manipulate.
pub mod world;

pub use add_component::AddComponent;
pub use add_distinct_component::AddDistinctComponent;
pub use add_entity::AddEntity;
#[doc(inline)]
pub use component::{Component, Unique};
pub use contains::Contains;
pub use delete::Delete;
pub use entity_id::EntityId;
pub use get::Get;
#[doc(inline)]
pub use iter::IntoIter;
pub use remove::Remove;
#[doc(inline)]
pub use scheduler::{IntoWorkload, Workload};
#[cfg(feature = "proc")]
pub use shipyard_proc::{Borrow, BorrowInfo, Component, IntoIter, Label, Unique, WorldBorrow};
pub use unique::UniqueStorage;
pub use views::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut,
    View, ViewMut,
};
#[doc(inline)]
pub use world::World;

// These modules are exported in the advanced module.
//
// There is no simpler way to only re-export modules in advanced.
use advanced::{
    atomic_refcell, get_component, get_unique, iter_component, reserve, system, tracking,
};

type ShipHashMap<K, V> = hashbrown::HashMap<K, V>;
#[doc(hidden)]
pub type ShipHashSet<V> = hashbrown::HashSet<V>;

#[cfg(feature = "std")]
fn std_thread_id_generator() -> u64 {
    use std::thread::ThreadId;

    let thread_id = std::thread::current().id();
    let thread_id: *const ThreadId = &thread_id;
    let thread_id: *const u64 = thread_id as _;

    unsafe { *thread_id }
}

/// Items that do not need to be present in lib.rs.
///
/// Don't feel bad for importing items from this module.\
/// It's only to keep lib.rs simple and not scare people.
pub mod advanced {
    // There is no simpler way to only re-export modules here.

    /// Inner lock similar to `RwLock`.
    #[path = "../atomic_refcell.rs"]
    pub mod atomic_refcell;
    /// Trait bound for [`AllStorages::get`](crate::all_storages::AllStorages::get) and [`crate::world::World::get`].
    #[path = "../get_component.rs"]
    pub mod get_component;
    /// Trait bound for [`AllStorages::get_unique`](crate::all_storages::AllStorages::get_unique) and [`crate::world::World::get_unique`].
    #[path = "../get_unique.rs"]
    pub mod get_unique;
    /// Trait used as bound for [`World::iter`](crate::world::World::iter) and [`AllStorages::iter`](crate::all_storages::AllStorages::iter).
    #[path = "../iter_component.rs"]
    pub mod iter_component;
    /// Reserves memory for a set of entities.
    #[path = "../reserve.rs"]
    pub mod reserve;
    /// Trait bound encompassing all functions that can be used as system.
    #[path = "../system/mod.rs"]
    pub mod system;
    /// Traits implementing all trackings.
    #[path = "../tracking.rs"]
    pub mod tracking;

    pub use crate::entities::Entities;
    pub use crate::r#mut::Mut;
    pub use crate::storage::{SBoxBuilder, Storage, StorageId};
    pub use crate::views::{
        UniqueOrDefaultView, UniqueOrDefaultViewMut, UniqueOrInitView, UniqueOrInitViewMut,
    };
}
