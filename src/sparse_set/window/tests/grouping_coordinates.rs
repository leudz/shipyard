// These are raw-window contract tests, not alternate iterator constructors.
// Keep direct resolution checks for local/absolute indices and timestamp
// access; traversal, cloning through splits, empty and ZST behavior use the
// public API in the iterator suites.
use super::super::{EntityCursor, FullRawWindow, FullRawWindowMut};
use crate::iter::Shiperator;
use crate::tracking::Modified;
use crate::{track, Component, EntityId, Get, View, ViewMut, World};
use alloc::vec::Vec;
use core::ptr::NonNull;

#[derive(Debug, PartialEq)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

#[derive(Debug, PartialEq)]
struct Tracked(u32);
impl Component for Tracked {
    type Tracking = track::Modification;
}

/// Builds six entities where entity `i` owns `A(i)` at index `i` of `A`.
/// Entities 2 to 5 also own `B(10 + i)` at index `i - 1` of `B`,
/// after an unrelated `B(100)` at index 0.
fn world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();

    world.add_entity(B(100));
    let entities: Vec<EntityId> = (0..6).map(|i| world.add_entity(A(i))).collect();
    for (i, &entity) in entities.iter().enumerate().skip(2) {
        world.add_component(entity, B(10 + i as u32));
    }

    (world, entities)
}

/// Returns a cursor over the `len` entities starting at index `start` of `dense`.
fn entities_from(dense: &[EntityId], start: usize, len: usize) -> EntityCursor {
    let segment = &dense[start..start + len];

    // SAFE: The storage `dense` belongs to stays borrowed while the tests use the cursor.
    unsafe { EntityCursor::new(NonNull::from(segment).cast(), len) }
}

/// Asserts that two windows with different nonzero segment starts resolve the same
/// local index to the components of the same entity, adding their start only once.
#[test]
fn local_indices() {
    let (world, _) = world();
    let a = world.borrow::<View<'_, A>>().unwrap();
    let b = world.borrow::<View<'_, B>>().unwrap();
    let mut entities = entities_from(&a.dense, 2, 4);

    let mut a_window = FullRawWindow::from_view(&a);
    let mut b_window = FullRawWindow::from_view(&b);
    // SAFE: Both segments are 4 components long and cover entities 2 to 5,
    // stored at 2..6 in A, at 1..5 in B and at 0..4 in `entities`.
    unsafe {
        a_window.set_segment_start(2);
        b_window.set_segment_start(1);
    }

    for local in 0..4 {
        assert_eq!(entities.local_index(), local);

        // SAFE: `local` is within the 4 components long segments
        // and is the local index of the current candidate of `entities`.
        let (a_component, b_component) = unsafe {
            let a_index = a_window.captain_indices_of(&entities, local).unwrap();
            let b_index = b_window.captain_indices_of(&entities, local).unwrap();

            (a_window.get_data(a_index), b_window.get_data(b_index))
        };

        assert_eq!(a_component, &A(2 + local as u32));
        assert_eq!(b_component, &B(12 + local as u32));

        entities.advance();
    }
}

/// Asserts that sparse lookups ignore the segment start and return absolute indices,
/// including for an entity stored before the segment.
#[test]
fn sailor_absolute() {
    let (world, entities) = world();
    let a = world.borrow::<View<'_, A>>().unwrap();

    let mut a_window = FullRawWindow::from_view(&a);
    // SAFE: The segment covers entities 4 and 5, stored at 4..6 in A.
    // The window is never used as a captain.
    unsafe {
        a_window.set_segment_start(4);
    }

    let index = a_window.sailor_indices_of(entities[1]).unwrap();
    assert_eq!(index, 1);
    // SAFE: `index` was returned by `sailor_indices_of`.
    assert_eq!(unsafe { a_window.get_data(index) }, &A(1));
}

/// Asserts that resolving indices and creating `Mut`s without writing through them
/// doesn't mark any component as modified.
#[test]
fn resolve_no_modify() {
    let mut world = World::new_for_test();
    let entities: Vec<EntityId> = (0..4).map(|i| world.add_entity(Tracked(i))).collect();
    let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
    let mut segment_entities = entities_from(&tracked.dense, 1, 3);

    let mut window = FullRawWindowMut::new(&mut tracked);
    // SAFE: The segment is 3 components long and covers entities 1 to 3,
    // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
    unsafe {
        window.set_segment_start(1);
    }

    for local in 0..3 {
        // SAFE: `local` is within the 3 components long segment
        // and is the local index of the current candidate of `segment_entities`.
        // Each `Mut` is dropped before the next one is created.
        unsafe {
            let index = window.captain_indices_of(&segment_entities, local).unwrap();
            let _ = window.get_data(index);
        }

        segment_entities.advance();
    }

    for entity in entities {
        assert!(!tracked.is_modified(entity));
    }
}

/// Asserts that a tracked window writes and reads the modification timestamp
/// of the resolved component, not the one at the unadjusted local index.
#[test]
fn tracked_resolution() {
    let mut world = World::new_for_test();
    let entities: Vec<EntityId> = (0..4).map(|i| world.add_entity(Tracked(i))).collect();
    let mut tracked = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
    let mut segment_entities = entities_from(&tracked.dense, 1, 3);

    let mut window = FullRawWindowMut::new(&mut tracked);
    // SAFE: The segment is 3 components long and covers entities 1 to 3,
    // stored at 1..4 in the storage and at 0..3 in `segment_entities`.
    unsafe {
        window.set_segment_start(1);
    }

    segment_entities.advance();
    // SAFE: Local index 1 is within the 3 components long segment
    // and is the local index of the current candidate of `segment_entities`.
    // No other reference to the component exists while `component` lives.
    unsafe {
        let index = window.captain_indices_of(&segment_entities, 1).unwrap();
        let mut component = window.get_data(index);
        component.0 = 42;
    }

    let modified = Modified(window);
    let mut segment_entities = entities_from(&tracked.dense, 1, 3);
    let modified_indices: Vec<_> = (0..3)
        .map(|local| {
            // SAFE: `local` is within the 3 components long segment
            // and is the local index of the current candidate of `segment_entities`.
            let index = unsafe { modified.captain_indices_of(&segment_entities, local) };
            segment_entities.advance();

            index
        })
        .collect();
    assert_eq!(modified_indices, [None, Some(2), None]);

    assert_eq!((&tracked).get(entities[2]).unwrap(), &Tracked(42));
    assert!(tracked.is_modified(entities[2]));
    assert!(!tracked.is_modified(entities[1]));
}
