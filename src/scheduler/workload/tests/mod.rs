mod batching;
mod conditions;
mod info_tests;
mod ordering;
mod requirements;

use super::*;
use crate::component::{Component, Unique};
use crate::scheduler::IntoWorkload;
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
