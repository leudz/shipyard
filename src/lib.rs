//! Shipyard is an Entity Component System focused on usability and speed.
//!
//! The [user guide](https://leudz.github.io/shipyard/guide) is a great place to learn all about Shipyard!
//!
//! ## Features
//!
//! - **serde1** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
//! - **thread_local** &mdash; add methods and types required to work with `!Send` and `!Sync` components
//! - **std** *(default)* &mdash; let shipyard use the standard library

#![deny(bare_trait_objects)]
#![deny(elided_lifetimes_in_paths)]
#![deny(trivial_casts)]
#![deny(trivial_numeric_casts)]
#![deny(unused_qualifications)]
#![deny(clippy::used_underscore_binding)]
#![deny(clippy::similar_names)]
#![deny(clippy::filter_map)]
#![deny(clippy::invalid_upcast_comparisons)]
#![deny(clippy::cast_precision_loss)]
#![deny(clippy::cast_possible_wrap)]
#![deny(clippy::mutex_integer)]
#![deny(clippy::mut_mut)]
#![deny(clippy::items_after_statements)]
#![deny(clippy::print_stdout)]
#![deny(clippy::maybe_infinite_iter)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

mod atomic_refcell;
/// Allows access to helper types needed to implement `Borrow`.
pub mod borrow;
mod delete;
pub mod error;
mod get;
mod not;
mod pack {
    pub(crate) mod update;
}
mod add_component;
mod add_entity;
mod all_storages;
mod contains;
mod entities;
mod entity_id;
pub mod iter;
mod memory_usage;
mod r#mut;
mod remove;
mod reserve;
mod scheduler;
mod sparse_set;
mod storage;
mod system;
mod type_id;
mod unique;
mod unknown_storage;
mod view;
mod world;

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
pub use add_entity::AddEntity;
pub use all_storages::{AllStorages, CustomStorageAccess};
pub use atomic_refcell::{ExclusiveBorrow, Ref, RefMut, SharedBorrow};
#[doc(inline)]
pub use borrow::{AllStoragesBorrow, Borrow, BorrowInfo, IntoBorrow, Mutability};
pub use contains::Contains;
pub use delete::Delete;
pub use entities::Entities;
pub use entity_id::EntityId;
pub use get::Get;
pub use iter::{IntoFastIter, IntoIter, IntoWithId};
pub use memory_usage::StorageMemoryUsage;
pub use not::Not;
pub use pack::update::{Inserted, InsertedOrModified, Modified};
pub use r#mut::Mut;
pub use remove::Remove;
pub use reserve::{BulkEntityIter, BulkReserve};
pub use scheduler::{info, IntoWorkloadSystem, Workload, WorkloadBuilder, WorkloadSystem};
pub use sparse_set::{SparseArray, SparseSet, SparseSetDrain};
pub use storage::StorageId;
#[doc(hidden)]
pub use system::{AllSystem, Nothing, System};
pub use unique::Unique;
pub use unknown_storage::UnknownStorage;
pub use view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
pub use world::World;
