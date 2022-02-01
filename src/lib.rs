//! Shipyard is an Entity Component System focused on usability and speed.
//!
//! The [user guide](https://leudz.github.io/shipyard/guide/master) is a great place to learn all about Shipyard!
//!
//! ## Features
//!
//! - **parallel** *(default)* &mdash; enables workload threading and add parallel iterators
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
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

extern crate alloc;

mod add_component;
mod add_distinct_component;
mod add_entity;
mod all_storages;
mod atomic_refcell;
/// Allows access to helper types needed to implement `Borrow`.
pub mod borrow;
mod component;
mod contains;
mod delete;
mod entities;
mod entity_id;
pub mod error;
mod get;
pub mod iter;
mod memory_usage;
mod r#mut;
mod not;
mod or;
mod public_transport;
mod remove;
mod reserve;
mod scheduler;
mod seal;
mod sparse_set;
mod storage;
mod system;
mod timestamp;
/// module related to storage tracking, like insertion or modification.
pub mod track;
mod tracking;
mod type_id;
mod unique;
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
pub use add_distinct_component::AddDistinctComponent;
pub use add_entity::AddEntity;
pub use all_storages::{AllStorages, CustomStorageAccess, TupleDeleteAny, TupleRetain};
#[doc(hidden)]
pub use atomic_refcell::{ExclusiveBorrow, SharedBorrow};
pub use atomic_refcell::{Ref, RefMut};
#[doc(inline)]
pub use borrow::{AllStoragesBorrow, Borrow, BorrowInfo, IntoBorrow, Mutability};
pub use component::Component;
pub use contains::Contains;
pub use delete::Delete;
pub use entities::Entities;
pub use entity_id::EntityId;
pub use get::Get;
pub use iter::{IntoIter, IntoWithId};
pub use memory_usage::StorageMemoryUsage;
pub use not::Not;
pub use or::{OneOfTwo, Or};
pub use r#mut::Mut;
pub use remove::Remove;
pub use reserve::{BulkEntityIter, BulkReserve};
pub use scheduler::{
    info, AsLabel, IntoWorkload, IntoWorkloadSystem, Label, ScheduledWorkload, Workload,
    WorkloadBuilder, WorkloadSystem,
};
#[cfg(feature = "proc")]
pub use shipyard_proc::{AllStoragesBorrow, Borrow, BorrowInfo, Component};
pub use sparse_set::{
    BulkAddEntity, SparseArray, SparseSet, SparseSetDrain, TupleAddComponent, TupleDelete,
    TupleRemove,
};
pub use storage::{Storage, StorageId};
#[doc(hidden)]
pub use system::{AllSystem, Nothing, System};
pub use timestamp::TrackingTimestamp;
pub use tracking::{Inserted, InsertedOrModified, Modified};
pub use unique::Unique;
pub use view::{
    AllStoragesView, AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut,
    View, ViewMut,
};
pub use world::World;
