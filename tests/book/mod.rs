use shipyard::{track, Component};

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

#[derive(Debug)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[derive(Debug)]
struct F32(f32);
impl Component for F32 {
    type Tracking = track::Untracked;
}
