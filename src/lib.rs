#![deny(bare_trait_objects)]

mod atomic_refcell;
mod component_storage;
mod entity;
mod error;
mod sparse_array;
mod world;

pub use world::World;
