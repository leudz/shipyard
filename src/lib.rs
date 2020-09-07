//! Shipyard is an Entity Component System focused on usability and speed.
//!
//! # Getting started
//!
//! The [user guide](https://leudz.github.io/shipyard/guide) is a great place to learn all about Shipyard!  
//! Here's two examples to get an idea of what it provides:
//! ```
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Position {
//!     _x: f32,
//!     _y: f32,
//! }
//!
//! fn in_acid(positions: View<Position>, mut healths: ViewMut<Health>) {
//!     for (_, mut health) in (&positions, &mut healths)
//!         .iter()
//!         .filter(|(pos, _)| is_in_acid(pos))
//!     {
//!         health.0 -= 1.0;
//!     }
//! }
//!
//! fn is_in_acid(_: &Position) -> bool {
//!     // it's wet season
//!     true
//! }
//!
//! let world = World::new();
//!
//! world.run(
//!     |mut entities: EntitiesViewMut,
//!      mut positions: ViewMut<Position>,
//!      mut healths: ViewMut<Health>| {
//!         entities.add_entity(
//!             (&mut positions, &mut healths),
//!             (Position { _x: 0.0, _y: 0.0 }, Health(1000.0)),
//!         );
//!     },
//! );
//!
//! world.run(in_acid);
//! ```
//! # Let's make some pigs!
//! ```
//! # #[cfg(feature = "parallel")]
//! # {
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Fat(f32);
//!
//! fn reproduction(
//!     mut fats: ViewMut<Fat>,
//!     mut healths: ViewMut<Health>,
//!     mut entities: EntitiesViewMut,
//! ) {
//!     let count = (&healths, &fats)
//!         .iter()
//!         .filter(|(health, fat)| health.0 > 40.0 && fat.0 > 20.0)
//!         .count();
//!
//!     for _ in 0..count {
//!         entities.add_entity((&mut healths, &mut fats), (Health(100.0), Fat(0.0)));
//!     }
//! }
//!
//! fn meal(mut fats: ViewMut<Fat>) {
//!     for slice in (&mut fats).iter().into_chunk(8).ok().unwrap() {
//!         for fat in slice {
//!             fat.0 += 3.0;
//!         }
//!     }
//! }
//!
//! fn age(mut healths: ViewMut<Health>, thread_pool: ThreadPoolView) {
//!     use rayon::prelude::ParallelIterator;
//!
//!     thread_pool.install(|| {
//!         (&mut healths).par_iter().for_each(|health| {
//!             health.0 -= 4.0;
//!         });
//!     });
//! }
//!
//! let world = World::new();
//!
//! world.run(
//!     |mut entities: EntitiesViewMut, mut healths: ViewMut<Health>, mut fats: ViewMut<Fat>| {
//!         for _ in 0..100 {
//!             entities.add_entity((&mut healths, &mut fats), (Health(100.0), Fat(0.0)));
//!         }
//!     },
//! );
//!
//! Workload::builder("Life")
//!     .with_system(system!(meal))
//!     .with_system(system!(age))
//!     .add_to_world(&world)
//!     .unwrap();
//! Workload::builder("Reproduction")
//!     .with_system(system!(reproduction))
//!     .add_to_world(&world)
//!     .unwrap();
//!
//! for day in 0..100 {
//!     if day % 6 == 0 {
//!         world.run_workload("Reproduction");
//!     }
//!     world.run_default();
//! }
//!
//! world.run(|healths: View<Health>| {
//!     // we've got some new pigs
//!     assert_eq!(healths.len(), 900);
//! });
//! # }
//! ```
//!
//! ## Features
//!
//! - **panic** *(default)* adds panicking functions
//! - **parallel** *(default)* &mdash; adds parallel iterators and dispatch
//! - **serde1** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
//! - **non_send** &mdash; add methods and types required to work with `!Send` components
//! - **non_sync** &mdash; add methods and types required to work with `!Sync` components
//! - **std** *(default)* &mdash; let shipyard use the standard library
//!
//! ## Unsafe
//!
//! This crate uses `unsafe` both because sometimes there's no way around it, and for performance gain.  
//! Releases should have all invocation of `unsafe` explained.  
//! If you find places where a safe alternative is possible without repercussion (small ones are sometimes acceptable) feel free to open an issue or a PR.

#![deny(bare_trait_objects)]
#![deny(elided_lifetimes_in_paths)]
#![deny(trivial_casts)]
#![deny(trivial_numeric_casts)]
#![deny(unused_qualifications)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

mod add_unique_macro;
mod atomic_refcell;
mod borrow;
mod delete;
mod entity_builder;
//#[cfg(feature = "serde1")]
//mod erased_serde;
/// Contains all error types.
pub mod error;
mod get;
mod iter;
mod not;
mod pack;
mod remove;
//#[cfg(feature = "serde1")]
//mod serde_setup;
mod sparse_set;
mod storage;
mod system;
mod system_macro;
mod type_id;
mod unknown_storage;
mod view;
mod world;

#[cfg(feature = "non_send")]
pub use crate::borrow::NonSend;
#[cfg(all(feature = "non_send", feature = "non_sync"))]
pub use crate::borrow::NonSendSync;
#[cfg(feature = "non_sync")]
#[cfg_attr(docsrs, doc(cfg(feature = "non_sync")))]
pub use crate::borrow::NonSync;
#[doc(hidden)]
pub use crate::borrow::{AllStoragesBorrow, Borrow};
#[doc(hidden)]
pub use add_unique_macro::{AddUnique, Wrap};
pub use borrow::FakeBorrow;
pub use delete::Delete;
pub use entity_builder::EntityBuilder;
pub use get::Get;
pub use iter::{
    iterators, CurrentId, Enumerate, ExactSizeShiperator, Filter, IntoIter, IntoIterIds, Map,
    Shiperator, WithId,
};
pub use not::Not;
pub use pack::{LoosePack, TightPack};
pub use remove::Remove;
//#[cfg(feature = "serde1")]
//pub use serde_setup::{GlobalDeConfig, GlobalSerConfig, SerConfig};
pub use sparse_set::{
    sort, sort::IntoSortable, AddComponentUnchecked, Contains, OldComponent, SparseSet, Window,
    WindowMut,
};
pub use storage::{AllStorages, DeleteAny, Entities, EntityId, StorageId, Unique};
#[doc(hidden)]
pub use system::{AllSystem, Nothing, System};
#[cfg(feature = "parallel")]
pub use view::ThreadPoolView;
pub use view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
pub use world::scheduler::info;
pub use world::{Workload, WorkloadBuilder, World};
