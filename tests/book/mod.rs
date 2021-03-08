mod add_components;
mod add_entity;
mod delete_components;
mod delete_entity;
mod get;
mod hierarchy;
mod iterators;
#[cfg(feature = "thread_local")]
mod non_send_sync;
#[cfg(feature = "parallel")]
mod parallelism;
mod remove_components;
mod run;
mod sparse_set;
mod syntactic_peculiarities;
mod systems;
mod uniques;
mod world;
mod world_insides;
