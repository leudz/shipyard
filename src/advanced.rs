// These items have nothing special, I would like to have them in lib.rs
// but this seems like the more robust way.

/// Inner lock similar to `RwLock`.
pub mod atomic_refcell;
/// Trait bound for [`AllStorages::get`](crate::all_storages::AllStorages::get) and [`crate::world::World::get`].
pub mod get_component;
/// Trait bound for [`AllStorages::get_unique`](crate::all_storages::AllStorages::get_unique) and [`crate::world::World::get_unique`].
pub mod get_unique;
/// Reserves memory for a set of entities.
pub mod reserve;
/// Trait bound encompassing all functions that can be used as system.
pub mod system;
/// Traits implementing all trackings.
pub mod tracking;

pub use crate::entities::Entities;
pub use crate::r#mut::Mut;
pub use crate::storage::{SBoxBuilder, Storage, StorageId};
pub use crate::views::{
    UniqueOrDefaultView, UniqueOrDefaultViewMut, UniqueOrInitView, UniqueOrInitViewMut,
};
