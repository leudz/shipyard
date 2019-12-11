//! # Getting started
//! ```
//! use shipyard::prelude::*;
//!
//! struct Health(f32);
//! struct Position { x: f32, y: f32 };
//!
//! #[system(InAcid)]
//! fn run(pos: &Position, mut health: &mut Health) {
//!     for (pos, health) in (pos, health).iter() {
//!         if is_in_acid(pos) {
//!             health.0 -= 1.0;
//!         }
//!     }
//! }
//!
//! fn is_in_acid(pos: &Position) -> bool {
//!     // it's wet season
//!      
//!     true
//! }
//!
//! let world = World::new::<(Position, Health)>();
//!
//! world.run::<(EntitiesMut, &mut Position, &mut Health), _>(|(mut entities, mut pos, mut health)| {
//!     entities.add_entity((&mut pos, &mut health), (Position { x: 0.0, y: 0.0 }, Health(1000.0)));
//! });
//!
//! world.add_workload("In acid", InAcid);
//! world.run_default();
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
//! fn run(fat: &mut Fat) {
//!     for slice in fat.iter().into_chunk(8).ok().unwrap() {
//!         for fat in slice {
//!             fat.0 += 3.0;
//!         }
//!     }
//! }
//!
//! #[system(Age)]
//! fn run(health: &mut Health, thread_pool: ThreadPool) {
//!     use rayon::prelude::ParallelIterator;
//!
//!     thread_pool.install(|| {
//!         health.par_iter().for_each(|health| {
//!             health.0 -= 4.0;
//!         });
//!     });
//! }
//!
//! let world = World::new::<(Health, Fat)>();
//!
//! world.run::<(EntitiesMut, &mut Health, &mut Fat), _>(|(mut entities, mut health, mut fat)| {
//!     (0..100).for_each(|_| {
//!         entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!     })
//! });
//!
//! world.add_workload("Life", (Meal, Age));
//! world.add_workload("Reproduction", Reproduction);
//!
//! for day in 0..100 {
//!     if day % 6 == 0 {
//!         world.run_workload("Reproduction");
//!     }
//!     world.run_default();
//! }
//!
//! world.run::<&Health, _>(|health| {
//!     // we've got some new pigs
//!     assert_eq!(health.len(), 900);
//! });
//! # }
//! ```

#![deny(bare_trait_objects)]

mod atomic_refcell;
pub mod error;
mod get;
pub mod internal;
mod iter;
mod not;
pub mod prelude;
mod remove;
mod run;
mod sparse_set;
mod storage;
mod unknown_storage;
mod world;

/// Type used to borrow the rayon::ThreadPool inside `World`.
#[cfg(feature = "parallel")]
pub struct ThreadPool;

/// Type used to access the value of a unique storage.
/// # Example:
/// ```
/// # use shipyard::prelude::*;
/// let world = World::default();
/// world.register_unique(0usize);
///
/// world.run::<Unique<&mut usize>, _>(|x| {
///     *x += 1;
/// });
/// ```
pub struct Unique<T: ?Sized>(T);
