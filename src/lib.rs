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
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod all_storages;
/// Inner lock similar to `RwLock`.
pub mod atomic_refcell;
/// Allows access to helper types needed to implement [`Borrow`](borrow::Borrow).
pub mod borrow;
mod component;
mod contains;
mod delete;
mod entities;
mod entity_id;
pub mod error;
mod get;
/// Trait bound for [`AllStorages::get`] and [`World::get`].
pub mod get_component;
/// Trait bound for [`AllStorages::get_unique`] and [`World::get_unique`].
pub mod get_unique;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod iter;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod iter_component;
/// Module describing internal memory usage.
pub mod memory_usage;
mod r#mut;
mod not;
mod optional;
mod or;
mod public_transport;
mod remove;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod reserve;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod scheduler;
mod seal;
/// Default component storage.
pub mod sparse_set;
mod storage;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod system;
/// Module related to storage tracking, like insertion or modification.
pub mod track;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod tracking;
mod type_id;
mod unique;
mod views;
#[allow(clippy::empty_docs)]
///
// We can't allow(missing_docs) without allowing it for everything inside
pub mod world;

#[cfg(feature = "thread_local")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use crate::borrow::NonSend;
#[cfg(feature = "thread_local")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use crate::borrow::NonSendSync;
#[cfg(feature = "thread_local")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use crate::borrow::NonSync;
pub use add_component::AddComponent;
pub use add_distinct_component::AddDistinctComponent;
pub use add_entity::AddEntity;
#[doc(inline)]
pub use all_storages::AllStorages;
pub use component::{Component, Unique};
pub use contains::Contains;
pub use delete::Delete;
pub use entities::Entities;
pub use entity_id::EntityId;
pub use get::Get;
#[doc(inline)]
pub use iter::IntoIter;
pub use not::Not;
pub use optional::Optional;
pub use or::{OneOfTwo, Or};
pub use r#mut::Mut;
pub use remove::Remove;
#[doc(inline)]
pub use scheduler::{
    IntoWorkload, IntoWorkloadSystem, IntoWorkloadTrySystem, SystemModificator, Workload,
    WorkloadModificator,
};
#[cfg(feature = "proc")]
pub use shipyard_proc::{Borrow, BorrowInfo, Component, IntoIter, Label, Unique, WorldBorrow};
pub use storage::{SBoxBuilder, Storage, StorageId};
#[doc(inline)]
pub use tracking::{Inserted, InsertedOrModified, Modified};
pub use unique::UniqueStorage;
pub use views::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueOrDefaultView,
    UniqueOrDefaultViewMut, UniqueOrInitView, UniqueOrInitViewMut, UniqueView, UniqueViewMut, View,
    ViewMut,
};
#[doc(inline)]
pub use world::World;

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
