//! # Getting started
//! ```
//! use shipyard::prelude::*;
//!
//! struct Health(f32);
//! struct Position { x: f32, y: f32 };
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
//! ```
//! # Let's make some pigs!
//! ```
//! # #[cfg(feature = "parallel")]
//! # {
//! use shipyard::prelude::*;
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

#![deny(bare_trait_objects)]
#![deny(elided_lifetimes_in_paths)]
#![deny(trivial_casts)]
#![deny(trivial_numeric_casts)]
#![deny(unused_qualifications)]
#![cfg_attr(not(any(feature = "std", test)), no_std)]

#[macro_use]
extern crate alloc;

mod atomic_refcell;
mod delete;
pub mod error;
mod get;
pub mod internal;
mod iter;
mod not;
mod pack;
pub mod prelude;
mod remove;
mod run;
mod sparse_set;
mod storage;
mod unknown_storage;
mod views;
mod world;

pub use storage::{AllStorages, Entities, EntitiesMut, EntityId};
pub use world::World;

/// Type used to borrow the rayon::ThreadPool inside `World`.
#[cfg(feature = "parallel")]
pub struct ThreadPool;

/// Type used to access the value of a unique storage.
/// # Example:
/// ```
/// # use shipyard::prelude::*;
/// let world = World::default();
/// world.add_unique(0usize);
///
/// world.run::<Unique<&mut usize>, _, _>(|mut x| {
///     *x += 1;
/// });
/// ```
pub struct Unique<T: ?Sized>(T);

#[cfg(feature = "non_send")]
pub struct NonSend<T: ?Sized>(T);

#[cfg(feature = "non_sync")]
pub struct NonSync<T: ?Sized>(T);

#[cfg(all(feature = "non_send", feature = "non_sync"))]
pub struct NonSendSync<T: ?Sized>(T);
