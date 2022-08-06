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

// ANCHOR: component_manual
#[derive(Debug)]
struct Pos(f32, f32);
impl Component for Pos {
    // We'll come back to this in a later chapter
    type Tracking = track::Untracked;
}

#[derive(Debug)]
struct Vel(f32, f32);
impl Component for Vel {
    type Tracking = track::Untracked;
}
// ANCHOR_END: component_manual

impl Pos {
    fn new() -> Pos {
        Pos(0.0, 0.0)
    }
}

impl Vel {
    fn new() -> Vel {
        Vel(0.0, 0.0)
    }
}

#[rustfmt::skip]
#[allow(unused)]
fn component_derive() {
// ANCHOR: component_derive
#[derive(Component, Debug)]
struct Pos(f32, f32);

#[derive(Component, Debug)]
struct Vel(f32, f32);
// ANCHOR_END: component_derive
}
