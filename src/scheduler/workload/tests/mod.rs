mod batching;
mod conditions;
mod info_tests;
mod ordering;
#[cfg(all(feature = "parallel", feature = "std"))]
mod parallel;
mod pending;
mod prepare;
mod registration;
mod regroup;
mod requirements;
#[cfg(all(feature = "tracing", feature = "std"))]
mod traced;

use super::*;
use crate::component::{Component, Unique};
use crate::scheduler::IntoWorkload;
use crate::scheduler::Step::System;
use crate::scheduler::{Batches, SystemModificator, WorkloadModificator};
use crate::views::{AllStoragesViewMut, UniqueView, UniqueViewMut, View, ViewMut};
use crate::world::World;

struct Usize(usize);
#[allow(unused)]
struct U32(u32);
#[allow(unused)]
struct U16(u16);

impl Component for Usize {
    type Tracking = crate::track::Untracked;
}
impl Component for U32 {
    type Tracking = crate::track::Untracked;
}
impl Component for U16 {
    type Tracking = crate::track::Untracked;
}
impl Unique for Usize {}
impl Unique for U32 {}
impl Unique for U16 {}

/// Grouped components, for regroup tests.
#[derive(Clone)]
struct A;
#[derive(Clone)]
struct B;
struct C;
struct D;

impl Component for A {
    type Tracking = crate::track::Untracked;
}
impl Component for B {
    type Tracking = crate::track::Untracked;
}
impl Component for C {
    type Tracking = crate::track::Untracked;
}
impl Component for D {
    type Tracking = crate::track::Untracked;
}

/// Returns `true` when every component of `T` was evaluated by a regroup.
fn evaluated<T: Component + Send + Sync>(world: &World) -> bool {
    let view = world.borrow::<View<'_, T>>().unwrap();

    view.groups.unevaluated_start == view.len()
}
