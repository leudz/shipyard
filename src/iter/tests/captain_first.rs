//! Single-captain fast path: cheap filters reject early; nested joins keep tuple order.

use super::grouping_fixtures::{A, B};
use crate::iter::{
    ConstructionMetadata, EntityCursor, IntoIter, IntoShiperator, RawEntityIdAccess, SegmentEntry,
    SegmentRow, Shiperator,
};
use crate::{Component, EntityId, Get, View, ViewMut, World, track};
use alloc::vec::Vec;
use core::cell::Cell;

struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::Modification;
}

/// Counts sparse lookups without changing query selection.
struct Count<'a, S>(S, &'a Cell<usize>);

// SAFE: Delegates all access and layout unchanged; only counts sailor lookups.
unsafe impl<'a, Q: IntoShiperator> IntoShiperator for Count<'a, Q> {
    type Shiperator = Count<'a, Q::Shiperator>;

    fn into_shiperator(self) -> (Self::Shiperator, RawEntityIdAccess, ConstructionMetadata) {
        let (inner, entities, metadata) = self.0.into_shiperator();
        (Count(inner, self.1), entities, metadata)
    }
    fn can_captain() -> bool {
        Q::can_captain()
    }
    fn can_sailor() -> bool {
        Q::can_sailor()
    }
}

impl<S: Shiperator> Shiperator for Count<'_, S> {
    type Out = S::Out;
    type Index = S::Index;

    const PREFER_CAPTAIN_FIRST: bool = S::PREFER_CAPTAIN_FIRST;

    fn node_count() -> usize {
        S::node_count()
    }
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.0.describe(active, row);
    }
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: The inner node layout is unchanged.
        unsafe { self.0.enter_segment(entry) };
    }
    fn sail_time(&self) -> usize {
        self.0.sail_time()
    }
    fn is_exact_sized(&self) -> bool {
        self.0.is_exact_sized()
    }
    fn storage_index(index: &Self::Index) -> Option<usize> {
        S::storage_index(index)
    }
    unsafe fn captain_indices_of(
        &self,
        entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        // SAFE: Forwards the caller's cursor and local index unchanged.
        unsafe { self.0.captain_indices_of(entities, index) }
    }
    fn sailor_indices_of(&self, id: EntityId) -> Option<Self::Index> {
        self.1.set(self.1.get() + 1);
        self.0.sailor_indices_of(id)
    }
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        // SAFE: The inner resolvers produced this index.
        unsafe { self.0.get_data(index) }
    }
}

#[test]
fn tracking_rejects_before_sailor_lookups() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4)
        .map(|i| world.add_entity((A(i), Tracked(i))))
        .collect();
    // Tracking costs twice its length; keep it the cheapest captain.
    for i in 4..12 {
        world.add_entity(A(i));
    }
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();
    let (a, mut tracked) = world
        .borrow::<(View<'_, A>, ViewMut<'_, Tracked>)>()
        .unwrap();
    (&mut tracked).get(ids[1]).unwrap().0 += 10;

    let lookups = Cell::new(0);
    for fold in [false, true] {
        lookups.set(0);
        let iter = (Count(&a, &lookups), tracked.modified()).iter();
        assert_eq!(iter.shiperator.single_captain, 1);
        let sum = if fold {
            iter.fold(0, |sum, (a, tracked)| sum + a.0 + tracked.0)
        } else {
            let mut sum = 0;
            for (a, tracked) in iter {
                sum += a.0 + tracked.0;
            }
            sum
        };
        assert_eq!(sum, 12);
        assert_eq!(lookups.get(), 1);
    }
}

#[test]
fn nested_captain_keeps_tuple_order() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4).map(|i| world.add_entity((A(i), B(i)))).collect();
    world.add_component(ids[1], Tracked(1));
    for i in 0..11 {
        world.add_entity(Tracked(i));
    }
    let (a, b, outer) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, Tracked>)>()
        .unwrap();

    let lookups = Cell::new(0);
    for fold in [false, true] {
        lookups.set(0);
        let iter = (Count(&outer, &lookups), (&a, Count(&b, &lookups))).iter();
        assert_eq!(iter.shiperator.single_captain, 1);
        let sum = if fold {
            iter.fold(0, |sum, (outer, (a, b))| sum + outer.0 + a.0 + b.0)
        } else {
            let mut sum = 0;
            for (outer, (a, b)) in iter {
                sum += outer.0 + a.0 + b.0;
            }
            sum
        };
        assert_eq!(sum, 3);
        // Four outer lookups, then one B lookup for the only match.
        assert_eq!(lookups.get(), 5);
    }
}
