//! Shipyard is an Entity Component System focused on usability and speed.
//!
//! # Getting started
//!
//! The [user guide](https://leudz.github.io/shipyard/book) is a great place to learn all about Shipyard!  
//! Here's two examples to get an idea of what it provides:
//! ```
//! # #[cfg(feature = "proc")]
//! # {
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Position { x: f32, y: f32 }
//!
//! #[system(InAcid)]
//! fn run(pos: &Position, mut health: &mut Health) {
//!     (&pos, &mut health).iter()
//!         .filter(|(pos, _)| is_in_acid(pos))
//!         .for_each(|(pos, mut health)| {
//!             health.0 -= 1.0;
//!         });
//! }
//!
//! fn is_in_acid(pos: &Position) -> bool {
//!     // it's wet season
//!     true
//! }
//!
//! let world = World::new();
//!
//! {
//!     let (mut entities, mut positions, mut healths) =
//!         world.borrow::<(EntitiesMut, &mut Position, &mut Health)>();
//!    
//!     entities.add_entity(
//!         (&mut positions, &mut healths),
//!         (Position { x: 0.0, y: 0.0 },
//!         Health(1000.0))
//!     );
//! }
//!
//! world.run_system::<InAcid>();
//! # }
//! ```
//! # Let's make some pigs!
//! ```
//! # #[cfg(all(feature = "parallel", feature = "proc"))]
//! # {
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Fat(f32);
//!
//! #[system(Reproduction)]
//! fn run(mut fat: &mut Fat, mut health: &mut Health, mut entities: &mut Entities) {
//!     let count = (&health, &fat).iter().filter(|(health, fat)| health.0 > 40.0 && fat.0 > 20.0).count();
//!     (0..count).for_each(|_| {
//!         entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!     });
//! }
//!
//! #[system(Meal)]
//! fn run(mut fat: &mut Fat) {
//!     (&mut fat).iter().into_chunk(8).ok().unwrap().for_each(|slice| {
//!         for fat in slice {
//!             fat.0 += 3.0;
//!         }
//!     });
//! }
//!
//! #[system(Age)]
//! fn run(mut health: &mut Health, thread_pool: ThreadPool) {
//!     use rayon::prelude::ParallelIterator;
//!
//!     thread_pool.install(|| {
//!         (&mut health).par_iter().for_each(|health| {
//!             health.0 -= 4.0;
//!         });
//!     });
//! }
//!
//! let world = World::new();
//!
//! world.run::<(EntitiesMut, &mut Health, &mut Fat), _, _>(|(mut entities, mut health, mut fat)| {
//!     (0..100).for_each(|_| {
//!         entities.add_entity(
//!             (&mut health, &mut fat),
//!             (Health(100.0), Fat(0.0))
//!         );
//!     })
//! });
//!
//! world.add_workload::<(Meal, Age), _>("Life");
//! world.add_workload::<Reproduction, _>("Reproduction");
//!
//! for day in 0..100 {
//!     if day % 6 == 0 {
//!         world.run_workload("Reproduction");
//!     }
//!     world.run_default();
//! }
//!
//! world.run::<&Health, _, _>(|health| {
//!     // we've got some new pigs
//!     assert_eq!(health.len(), 900);
//! });
//! # }
//! ```
//!
//! ## Features
//!
//! - **parallel** *(default)* &mdash; adds parallel iterators and dispatch
//! - **proc** &mdash; adds `system` proc macro
//! - **serde** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
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

mod atomic_refcell;
mod borrow;
mod delete;
/// Contains all error types.
pub mod error;
mod get;
mod iter;
mod not;
mod pack;
mod remove;
mod sparse_set;
mod storage;
mod system_macro;
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
pub use borrow::FakeBorrow;
pub use delete::Delete;
pub use get::GetComponent;
pub use iter::{
    iterators, CurrentId, Enumerate, ExactSizeShiperator, Filter, IntoIter, IntoIterIds, Map,
    Shiperator, WithId,
};
pub use not::Not;
pub use pack::{LoosePack, TightPack};
pub use remove::Remove;
#[doc(hidden)]
#[cfg(feature = "proc")]
pub use shipyard_proc::system as system_fn;
pub use sparse_set::SparseSet;
pub use sparse_set::{sort, sort::IntoSortable, Window, WindowMut};
pub use storage::{AllStorages, Entities, EntitiesMut, EntityId};
#[cfg(feature = "parallel")]
pub use view::ThreadPoolView;
pub use view::{
    AllStoragesViewMut, EntitiesView, EntitiesViewMut, UniqueView, UniqueViewMut, View, ViewMut,
};
#[doc(hidden)]
pub use world::WorkloadBuilder;
pub use world::World;
