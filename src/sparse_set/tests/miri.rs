//! Focused Miri scenarios for `SparseSet` storage and ownership.
//!
//! The sparse set indexes its sparse pages, dense ids, component data and timestamps unchecked,
//! and moves components between rows. Components here own heap memory and count their drops,
//! so a lost, duplicated or misplaced component shows up as a wrong value, a wrong count or a
//! Miri error.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::Shiperator;
use crate::remove::Remove;
use crate::sparse_set::{
    EntityCursor, FullRawWindow, FullRawWindowMut, GroupRange, InsertionResult, PAGE_LEN, SparseSet,
};
use crate::storage::StorageId;
use crate::track;
use crate::tracking::TrackingTimestamp;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Heap-owning component that counts how many times it's dropped.
struct Item {
    values: Vec<u32>,
    drops: Arc<AtomicUsize>,
}

impl Drop for Item {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}

impl Component for Item {
    type Tracking = track::Untracked;
}

/// `Item` with every kind of tracking enabled.
struct TrackedItem(Item);

impl Component for TrackedItem {
    type Tracking = track::All;
}

/// Component aligned more strictly than any of its fields need.
#[repr(align(64))]
struct Aligned(u64);

impl Component for Aligned {
    type Tracking = track::Untracked;
}

struct Zst;

impl Component for Zst {
    type Tracking = track::Untracked;
}

fn counter() -> Arc<AtomicUsize> {
    Arc::new(AtomicUsize::new(0))
}

fn drops(counter: &Arc<AtomicUsize>) -> usize {
    counter.load(Ordering::Relaxed)
}

fn item(value: u32, drops: &Arc<AtomicUsize>) -> Item {
    Item {
        values: vec![value],
        drops: drops.clone(),
    }
}

/// Returns the id at `index` with generation 0, it doesn't need to come from an `Entities`.
fn id(index: usize) -> EntityId {
    EntityId::new_from_index_and_gen(index as u64, 0)
}

fn now() -> TrackingTimestamp {
    TrackingTimestamp::new(1)
}

/// Returns the values of `entity`'s component, panics if it doesn't have one.
fn values_of(sparse_set: &SparseSet<Item>, entity: EntityId) -> &[u32] {
    &sparse_set.private_get(entity).unwrap().values
}

/// Asserts sparse page allocation around page boundaries: components at the last index of page 0,
/// the first of page 1 and in page 3 are reachable, an id in the never allocated page 2 isn't,
/// and the swap-remove moving the page 3 component updates its sparse entry.
#[test]
fn page_boundaries() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    let last_of_page0 = id(PAGE_LEN - 1);
    let first_of_page1 = id(PAGE_LEN);
    let in_page2 = id(2 * PAGE_LEN);
    let in_page3 = id(3 * PAGE_LEN + 1);

    for (entity, value) in [(last_of_page0, 0), (first_of_page1, 1), (in_page3, 3)] {
        sparse_set
            .insert(entity, item(value, &counter), now())
            .assert_inserted();
    }
    assert_eq!(sparse_set.sparse.len(), 4);
    assert!(sparse_set.sparse.get(in_page2).is_none());
    assert!(sparse_set.private_get(in_page2).is_none());

    // The page 0 component is first in dense, removing it moves the page 3 one into its slot.
    let removed = sparse_set.dyn_remove(last_of_page0, now()).unwrap();
    assert_eq!(removed.values, [0]);
    assert_eq!(sparse_set.index_of(in_page3), Some(0));
    assert_eq!(values_of(&sparse_set, in_page3), [3]);
    assert_eq!(values_of(&sparse_set, first_of_page1), [1]);

    drop(removed);
    drop(sparse_set);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that components keep their values and stay reachable when inserting reallocates the
/// dense and data vectors.
#[test]
fn dense_growth() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    sparse_set
        .insert(id(0), item(0, &counter), now())
        .assert_inserted();
    let capacity = sparse_set.data.capacity();

    for index in 1..=capacity {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }
    assert!(sparse_set.data.capacity() > capacity);

    for index in 0..=capacity {
        assert_eq!(values_of(&sparse_set, id(index)), [index as u32]);
    }
}

/// Asserts the three ways `insert` can meet an existing component: the same generation hands the
/// old component back, a newer generation drops it in place, an older generation is rejected and
/// the new value dropped. Every value is dropped exactly once.
#[test]
fn overwrite() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    let entity = id(0);
    let newer = EntityId::new_from_index_and_gen(0, 1);
    sparse_set
        .insert(entity, item(0, &counter), now())
        .assert_inserted();

    let result = sparse_set.insert(entity, item(1, &counter), now());
    let InsertionResult::ComponentOverride(old) = result else {
        panic!("the same generation should hand the old component back");
    };
    assert_eq!(old.values, [0]);
    drop(old);
    assert_eq!(drops(&counter), 1);

    let result = sparse_set.insert(newer, item(2, &counter), now());
    assert!(matches!(result, InsertionResult::OtherComponentOverride));
    assert_eq!(drops(&counter), 2);
    assert!(sparse_set.private_get(entity).is_none());
    assert_eq!(values_of(&sparse_set, newer), [2]);

    let result = sparse_set.insert(entity, item(3, &counter), now());
    assert!(matches!(result, InsertionResult::NotInserted));
    assert_eq!(drops(&counter), 3);
    assert_eq!(values_of(&sparse_set, newer), [2]);

    drop(sparse_set);
    assert_eq!(drops(&counter), 4);
}

/// Asserts swap-removal from the middle, from the end and of the only component: the removed
/// value is handed back, the component moved into the hole stays reachable at its new index, and
/// a newer generation of the last id removes the stale component without handing it back.
#[test]
fn removal() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }

    let middle = sparse_set.dyn_remove(id(1), now()).unwrap();
    assert_eq!(middle.values, [1]);
    assert_eq!(sparse_set.index_of(id(2)), Some(1));
    assert_eq!(values_of(&sparse_set, id(2)), [2]);

    let last = sparse_set.dyn_remove(id(2), now()).unwrap();
    assert_eq!(last.values, [2]);
    assert_eq!(values_of(&sparse_set, id(0)), [0]);

    let newer = EntityId::new_from_index_and_gen(0, 1);
    assert!(sparse_set.dyn_remove(newer, now()).is_none());
    assert!(sparse_set.is_empty());
    assert_eq!(drops(&counter), 1);

    drop((middle, last));
    assert_eq!(drops(&counter), 3);
}

/// Asserts that deleting from a deletion-tracking storage keeps the component alive in the
/// deletion data, unlike removal which hands it back, and that clearing the deletion data drops
/// it exactly once.
#[test]
fn deletion_tracking() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..2 {
        sparse_set
            .insert(id(index), TrackedItem(item(index as u32, &counter)), now())
            .assert_inserted();
    }

    assert!(sparse_set.dyn_delete(id(0), now()));
    assert_eq!(drops(&counter), 0);
    assert_eq!(sparse_set.deletion_data[0].2.0.values, [0]);

    let removed = sparse_set.dyn_remove(id(1), now()).unwrap();
    assert_eq!(sparse_set.removal_data.len(), 1);
    drop(removed);
    assert_eq!(drops(&counter), 1);

    sparse_set.clear_all_deleted();
    assert_eq!(drops(&counter), 2);
}

/// Asserts that `clear` drops every component exactly once in a plain storage and moves them to
/// the deletion data in a deletion-tracking one, and that both storages are empty and reusable
/// afterwards. Clearing an empty storage does nothing.
#[test]
fn clear() {
    let counter = counter();

    let mut plain = SparseSet::new();
    plain.private_clear(now());
    for index in 0..2 {
        plain
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }
    plain.private_clear(now());
    assert_eq!(drops(&counter), 2);
    assert!(plain.private_get(id(0)).is_none());
    plain
        .insert(id(1), item(9, &counter), now())
        .assert_inserted();
    assert_eq!(values_of(&plain, id(1)), [9]);

    let mut tracked = SparseSet::new();
    for index in 0..2 {
        tracked
            .insert(id(index), TrackedItem(item(index as u32, &counter)), now())
            .assert_inserted();
    }
    tracked.private_clear(now());
    assert_eq!(drops(&counter), 2);
    assert!(tracked.is_empty());
    assert_eq!(tracked.deletion_data.len(), 2);

    tracked.clear_all_deleted();
    assert_eq!(drops(&counter), 4);
}

/// Asserts that dropping a partially consumed drain drops the components it didn't yield, while
/// the one it yielded stays ours and valid, and that the storage is empty and reusable.
#[test]
fn drain_partial() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }

    let mut drain = sparse_set.private_drain(now());
    let first = drain.next().unwrap();
    drop(drain);
    assert_eq!(drops(&counter), 2);
    assert_eq!(first.values, [0]);

    assert!(sparse_set.is_empty());
    assert!(sparse_set.private_get(id(1)).is_none());
    sparse_set
        .insert(id(1), item(9, &counter), now())
        .assert_inserted();
    assert_eq!(values_of(&sparse_set, id(1)), [9]);

    drop((first, sparse_set));
    assert_eq!(drops(&counter), 4);
}

/// Asserts that draining with ids reads each id from the dense vector the drain already emptied,
/// paired with the right component, and that draining an empty storage yields nothing.
#[test]
fn drain_with_id() {
    let counter = counter();
    let mut sparse_set = SparseSet::<Item>::new();
    assert!(sparse_set.private_drain(now()).with_id().next().is_none());

    sparse_set
        .insert(id(3), item(3, &counter), now())
        .assert_inserted();
    sparse_set
        .insert(id(7), item(7, &counter), now())
        .assert_inserted();

    let mut drain = sparse_set.private_drain(now()).with_id();
    let (entity, first) = drain.next().unwrap();
    assert_eq!((entity, first.values.as_slice()), (id(3), &[3][..]));
    let (entity, second) = drain.next().unwrap();
    assert_eq!((entity, second.values.as_slice()), (id(7), &[7][..]));
    assert!(drain.next().is_none());
}

/// Asserts that `retain_mut` hands each callback a mutable reference to the right component
/// while earlier rejections swap later components down, that `retain` then reads the moved
/// components correctly, and that every rejected component is dropped exactly once.
#[test]
fn retain() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..4 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }

    sparse_set.private_retain_mut(now(), |entity, mut component| {
        component.values.push(10);
        entity.index() % 2 == 0
    });
    assert_eq!(drops(&counter), 2);
    assert_eq!(values_of(&sparse_set, id(0)), [0, 10]);
    assert_eq!(values_of(&sparse_set, id(2)), [2, 10]);

    sparse_set.private_retain(now(), |entity, component| {
        assert_eq!(component.values[0] as u64, entity.index());
        entity == id(2)
    });
    assert_eq!(drops(&counter), 3);
    assert_eq!(sparse_set.index_of(id(2)), Some(0));
}

/// Asserts that a `retain_mut` callback unwinding partway leaves the storage consistent: the
/// component it already rejected is gone and dropped once, the others are all still reachable.
#[cfg(feature = "std")]
#[test]
fn retain_panic() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.private_retain_mut(now(), |entity, _| {
            if entity == id(0) {
                return false;
            }
            panic!("retain callback panicked");
        });
    }));
    assert!(result.is_err());
    assert_eq!(drops(&counter), 1);
    assert_eq!(sparse_set.len(), 2);
    assert_eq!(values_of(&sparse_set, id(1)), [1]);
    assert_eq!(values_of(&sparse_set, id(2)), [2]);

    drop(sparse_set);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that `apply_mut` hands out mutable references to two distinct rows at the same time
/// and `apply` a mutable and a shared one, and that naming the same row twice panics before any
/// aliasing reference exists, leaving the storage usable.
#[cfg(feature = "std")]
#[test]
fn apply() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..2 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }

    sparse_set.private_apply_mut(
        id(0),
        id(1),
        |a, b| core::mem::swap(&mut a.values, &mut b.values),
        now(),
    );
    sparse_set.private_apply(
        id(0),
        id(1),
        |a, b| a.values.extend_from_slice(&b.values),
        now(),
    );
    assert_eq!(values_of(&sparse_set, id(0)), [1, 0]);
    assert_eq!(values_of(&sparse_set, id(1)), [0]);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.private_apply_mut(id(0), id(0), |_, _| {}, now());
    }));
    assert!(result.is_err());
    assert_eq!(values_of(&sparse_set, id(0)), [1, 0]);
}

/// Asserts that an insertion callback unwinding drops the value being inserted exactly once and
/// leaves the storage without a component for that entity.
#[cfg(feature = "std")]
#[test]
fn insertion_callback_panic() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    sparse_set.on_insertion(|_, _| panic!("insertion callback panicked"));

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = sparse_set.insert(id(0), item(0, &counter), now());
    }));
    assert!(result.is_err());
    assert_eq!(drops(&counter), 1);
    assert!(sparse_set.is_empty());
    assert!(sparse_set.private_get(id(0)).is_none());

    let _ = sparse_set.take_on_insertion();
    sparse_set
        .insert(id(0), item(1, &counter), now())
        .assert_inserted();
    assert_eq!(values_of(&sparse_set, id(0)), [1]);
}

/// Asserts that a removal callback unwinding, after the component already left the storage,
/// drops that component exactly once while the storage stays consistent: the component swapped
/// into the hole is still reachable.
#[cfg(feature = "std")]
#[test]
fn removal_callback_panic() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(id(index), item(index as u32, &counter), now())
            .assert_inserted();
    }
    sparse_set.on_removal(|_, _| panic!("removal callback panicked"));

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = sparse_set.dyn_remove(id(0), now());
    }));
    assert!(result.is_err());
    assert_eq!(drops(&counter), 1);
    assert_eq!(sparse_set.len(), 2);
    assert_eq!(sparse_set.index_of(id(2)), Some(0));
    assert_eq!(values_of(&sparse_set, id(2)), [2]);
    assert_eq!(values_of(&sparse_set, id(1)), [1]);

    let _ = sparse_set.take_on_removal();
    drop(sparse_set);
    assert_eq!(drops(&counter), 3);
}

/// Asserts that swap-removal and draining move over-aligned components between rows, every
/// access stays aligned and each component keeps its value.
#[test]
fn over_aligned() {
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set
            .insert(id(index), Aligned(index as u64), now())
            .assert_inserted();
    }

    let removed = sparse_set.dyn_remove(id(0), now()).unwrap();
    assert_eq!(removed.0, 0);
    let moved = sparse_set.private_get(id(2)).unwrap();
    assert_eq!(moved.0, 2);
    assert_eq!(core::ptr::from_ref(moved) as usize % 64, 0);

    let drained: Vec<u64> = sparse_set
        .private_drain(now())
        .map(|aligned| aligned.0)
        .collect();
    assert_eq!(drained, [2, 1]);
}

/// Asserts that zero-sized components go through insertion, swap-removal and draining with ids,
/// even though their data vector never allocates.
#[test]
fn zero_sized() {
    let mut sparse_set = SparseSet::new();
    for index in 0..3 {
        sparse_set.insert(id(index), Zst, now()).assert_inserted();
    }

    assert!(sparse_set.dyn_remove(id(0), now()).is_some());
    assert_eq!(sparse_set.index_of(id(2)), Some(0));

    let drained: Vec<EntityId> = sparse_set
        .private_drain(now())
        .with_id()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(drained, [id(2), id(1)]);
}

/// Asserts that insertion and modification timestamps stay paired with their component when
/// sorting reorders rows and a removal swaps the last row into a hole: afterwards only the
/// component we modified reads as modified, and only the one inserted after clearing reads as
/// inserted.
#[test]
fn tracked_reorder() {
    let counter = counter();
    let mut world = World::new_for_test();
    let e0 = world.add_entity(TrackedItem(item(2, &counter)));
    let e1 = world.add_entity(TrackedItem(item(1, &counter)));
    world.run(|items: ViewMut<'_, TrackedItem, track::All>| items.clear_all_inserted());
    let e2 = world.add_entity(TrackedItem(item(0, &counter)));
    let e3 = world.add_entity(TrackedItem(item(3, &counter)));

    let mut items = world
        .borrow::<ViewMut<'_, TrackedItem, track::All>>()
        .unwrap();
    (&mut items).get(e1).unwrap().0.values.push(10);

    // Sorted by first value: e2, e1, e0, e3. Removing e2 then moves e3 to the front.
    items.sort_unstable_by(|a, b| a.0.values[0].cmp(&b.0.values[0]));
    assert_eq!(items.index_of(e2), Some(0));
    let removed = items.remove(e2).unwrap();
    assert_eq!(items.index_of(e3), Some(0));

    let states: Vec<(bool, bool)> = [e0, e1, e3]
        .into_iter()
        .map(|entity| (items.is_inserted(entity), items.is_modified(entity)))
        .collect();
    assert_eq!(states, [(false, false), (false, true), (true, false)]);
    assert_eq!((&items).get(e1).unwrap().0.values, [1, 10]);
    assert_eq!((&items).get(e3).unwrap().0.values, [3]);

    drop(items);
    drop(world);
    drop(removed);
    assert_eq!(drops(&counter), 4);
}

/// Asserts that bulk insertion fills sparse entries across a page boundary for both the single
/// component and the tuple paths. Spawning first moves the next entity index right before a
/// boundary, so we don't need hundreds of live entities to reach it.
#[test]
fn bulk_page_boundary() {
    let counter = counter();
    let mut world = World::new_for_test();

    world.spawn(id(PAGE_LEN - 2));
    let pairs: Vec<EntityId> = world
        .bulk_add_entity((0..2).map(|value| (item(value, &counter), Aligned(value as u64))))
        .collect();
    world.spawn(id(2 * PAGE_LEN - 2));
    let singles: Vec<EntityId> = world
        .bulk_add_entity((10..12).map(|value| item(value, &counter)))
        .collect();
    assert_ne!(pairs[0].page_index(), pairs[1].page_index());
    assert_ne!(singles[0].page_index(), singles[1].page_index());

    let (items, aligned) = world
        .borrow::<(View<'_, Item>, View<'_, Aligned>)>()
        .unwrap();
    for (value, &entity) in [0, 1, 10, 11].into_iter().zip(pairs.iter().chain(&singles)) {
        assert_eq!(items.get(entity).unwrap().values, [value]);
    }
    assert_eq!(aligned.get(pairs[1]).unwrap().0, 1);

    drop((items, aligned));
    drop(world);
    assert_eq!(drops(&counter), 4);
}

/// Returns a cursor over `entities`.
fn cursor_over(entities: &[EntityId]) -> EntityCursor {
    // SAFE: `entities` is a live slice, and every test keeps it alive and unmodified for as long
    // as it uses the cursor.
    unsafe { EntityCursor::new(NonNull::from(entities).cast(), entities.len()) }
}

/// Asserts the cursor's empty and single entity states over a live slice: an empty cursor, built
/// empty or from a zero-length range, reports nothing and never moves, so nothing ever reads its
/// dangling pointer. A single entity cursor reads its entity once, then stays exhausted.
#[test]
fn cursor_empty_and_single() {
    let entities = [id(4)];
    // SAFE: `len` is 0, so the pointer is never read.
    let zero_len = unsafe { EntityCursor::new(NonNull::from(&entities).cast(), 0) };

    for mut cursor in [EntityCursor::empty(), zero_len] {
        assert!(cursor.is_empty());
        assert_eq!(cursor.len(), 0);
        assert!(!cursor.advance());
    }

    let mut single = cursor_over(&entities);
    // SAFE: The cursor holds one entity.
    assert_eq!(unsafe { single.current() }, id(4));
    assert!(single.advance());
    assert!(single.is_empty());
    assert!(!single.advance());
    assert_eq!(single.local_index(), 1);
}

/// Asserts that splitting a partially consumed cursor at either end and in the middle hands out
/// halves over the right entities, keeping the original local indices, and that a segment taken
/// from it restarts them. Only non-empty halves are read.
#[test]
fn cursor_split() {
    let entities: Vec<EntityId> = (0..5).map(id).collect();
    let mut cursor = cursor_over(&entities);
    cursor.advance();

    for index in [0, 2, 4] {
        let (left, right) = cursor.split_at(index);
        assert_eq!((left.local_index(), left.len()), (1, index));
        assert_eq!((right.local_index(), right.len()), (1 + index, 4 - index));

        if !left.is_empty() {
            // SAFE: `left` isn't empty.
            assert_eq!(unsafe { left.current() }, entities[1]);
        }
        if !right.is_empty() {
            // SAFE: `right` isn't empty.
            assert_eq!(unsafe { right.current() }, entities[1 + index]);
        }
    }

    let segment = cursor.segment(1, 3);
    assert_eq!((segment.local_index(), segment.len()), (0, 3));
    // SAFE: The segment holds 3 entities.
    assert_eq!(unsafe { segment.current() }, entities[2]);
}

/// Asserts that windows with different nonzero segment starts resolve the same local index to
/// the components of the same entity, and that sparse lookups ignore the segment start.
#[test]
fn window_segment_start() {
    let counter = counter();
    let mut world = World::new_for_test();
    // `Aligned` gets an extra component first, so its segment starts one index later.
    world.add_entity(Aligned(100));
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((item(i, &counter), Aligned(i as u64))))
        .collect();
    let (items, aligned) = world
        .borrow::<(View<'_, Item>, View<'_, Aligned>)>()
        .unwrap();

    let mut item_window = FullRawWindow::from_view(&items);
    let mut aligned_window = FullRawWindow::from_view(&aligned);
    let mut cursor = item_window.entity_iter().cursor.segment(1, 3);
    // SAFE: Both segments are 3 components long and cover entities 1 to 3, stored at 1..4 in
    // `items`, at 2..5 in `aligned` and at local indices 0..3 in `cursor`.
    unsafe {
        item_window.set_segment_start(1);
        aligned_window.set_segment_start(2);
    }

    for local in 0..3 {
        // SAFE: `cursor` isn't empty, `local` is the local index of its current candidate and
        // it's within both segments.
        let (item, aligned) = unsafe {
            let item_index = item_window.captain_indices_of(&cursor, local).unwrap();
            let aligned_index = aligned_window.captain_indices_of(&cursor, local).unwrap();

            (
                item_window.get_data(item_index),
                aligned_window.get_data(aligned_index),
            )
        };
        assert_eq!(item.values, [local as u32 + 1]);
        assert_eq!(aligned.0, local as u64 + 1);

        cursor.advance();
    }

    let index = item_window.sailor_indices_of(e[0]).unwrap();
    assert_eq!(index, 0);
    // SAFE: `index` was just returned by `sailor_indices_of`.
    assert_eq!(unsafe { item_window.get_data(index) }.values, [0]);
}

/// Asserts that a tracked mutable window at a nonzero segment start writes the component and the
/// modification timestamp of the resolved entity, not the ones at the unadjusted local index.
#[test]
fn window_mut_tracked() {
    let counter = counter();
    let mut world = World::new_for_test();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity(TrackedItem(item(i, &counter))))
        .collect();
    let mut tracked = world
        .borrow::<ViewMut<'_, TrackedItem, track::All>>()
        .unwrap();

    {
        let mut window = FullRawWindowMut::new(&mut tracked);
        let mut cursor = window.entity_iter().cursor.segment(1, 3);
        // SAFE: The segment is 3 components long and covers entities 1 to 3, stored at 1..4 in
        // the storage and at local indices 0..3 in `cursor`.
        unsafe { window.set_segment_start(1) };
        cursor.advance();

        // SAFE: `cursor` isn't empty, local index 1 is its current candidate's and it's within
        // the segment. `component` is the only reference to this component while it lives.
        let index = unsafe {
            let index = window.captain_indices_of(&cursor, 1).unwrap();
            let mut component = window.get_data(index);
            component.0.values.push(10);

            index
        };
        assert_eq!(index, 2);

        // SAFE: Indices 1 to 3 are within the storage's 4 components.
        let modified: Vec<bool> = (1..4)
            .map(|index| unsafe { window.is_modified(index) })
            .collect();
        assert_eq!(modified, [false, true, false]);
    }

    assert_eq!((&tracked).get(e[2]).unwrap().0.values, [2, 10]);
    assert!(tracked.is_modified(e[2]));
    assert!(!tracked.is_modified(e[1]));
}

/// Places the components of `sparse_set`, in physical order, in back to back fully trusted ranges
/// of `(len, shift)`, then `ungrouped_len` of them in the ungrouped region. The rest is unevaluated.
fn group<T: Component>(
    sparse_set: &mut SparseSet<T>,
    ranges: &[(usize, isize)],
    ungrouped_len: usize,
) {
    let mut start = 0;
    for (position, &(len, shift)) in ranges.iter().enumerate() {
        sparse_set.groups.ranges.push(GroupRange {
            group: Arc::from([StorageId::Custom(0), StorageId::Custom(position as u64 + 1)]),
            start,
            len,
            shift,
            trust: len,
        });
        start += len;
    }
    sparse_set.groups.unevaluated_start = start + ungrouped_len;
}

/// Panics unless every entity in `dense` resolves to its own row and the regions are valid.
fn assert_consistent<T: Component>(sparse_set: &SparseSet<T>) {
    for (index, &entity) in sparse_set.dense.iter().enumerate() {
        assert_eq!(sparse_set.index_of(entity), Some(index));
    }

    #[cfg(debug_assertions)]
    sparse_set.groups.assert_invariants(sparse_set.len());
}

/// Builds a storage of `count` items whose values are their entity's index, in two rotated
/// ranges of 3 and 2 components, then 1 ungrouped component. The rest is unevaluated.
fn grouped_items(count: usize, counter: &Arc<AtomicUsize>) -> SparseSet<Item> {
    let mut sparse_set = SparseSet::new();
    for index in 0..count {
        sparse_set
            .insert(id(index), item(index as u32, counter), now())
            .assert_inserted();
    }
    group(&mut sparse_set, &[(3, 1), (2, -1)], 1);

    sparse_set
}

/// Asserts that removing from each region of a grouped storage, and deleting from one of its
/// ranges, hands back or drops the right component exactly once while every other one stays
/// reachable with its value. A newer generation removes the stale component without handing it back.
#[test]
fn grouped_removal() {
    let counter = counter();
    let mut sparse_set = grouped_items(8, &counter);

    for index in [1, 4, 5, 7] {
        let removed = sparse_set.dyn_remove(id(index), now()).unwrap();
        assert_eq!(removed.values, [index as u32]);
        assert_consistent(&sparse_set);
    }
    assert_eq!(drops(&counter), 4);

    assert!(sparse_set.dyn_delete(id(0), now()));
    assert_eq!(drops(&counter), 5);
    let newer = EntityId::new_from_index_and_gen(3, 1);
    assert!(sparse_set.dyn_remove(newer, now()).is_none());
    assert_eq!(drops(&counter), 6);
    assert_consistent(&sparse_set);

    for index in [2, 6] {
        assert_eq!(values_of(&sparse_set, id(index)), [index as u32]);
    }
    drop(sparse_set);
    assert_eq!(drops(&counter), 8);
}

/// Asserts that deleting from a range of a deletion-tracking grouped storage keeps the component
/// alive in the deletion data, and that clearing the deletion data drops it exactly once.
#[test]
fn grouped_deletion_tracking() {
    let counter = counter();
    let mut sparse_set = SparseSet::new();
    for index in 0..4 {
        sparse_set
            .insert(id(index), TrackedItem(item(index as u32, &counter)), now())
            .assert_inserted();
    }
    group(&mut sparse_set, &[(3, 2)], 0);

    assert!(sparse_set.dyn_delete(id(1), now()));
    assert_eq!(drops(&counter), 0);
    assert_eq!(sparse_set.deletion_data[0].2.0.values, [1]);
    assert_consistent(&sparse_set);

    sparse_set.clear_all_deleted();
    assert_eq!(drops(&counter), 1);
}

/// Asserts that `retain_mut` and `retain` on a grouped storage hand each callback the right
/// component while rejections move components across regions, and that every rejected component
/// is dropped exactly once.
#[test]
fn grouped_retain() {
    let counter = counter();
    let mut sparse_set = grouped_items(8, &counter);

    sparse_set.private_retain_mut(now(), |entity, mut component| {
        assert_eq!(component.values[0] as u64, entity.index());
        component.values.push(10);
        entity.index() % 2 == 0
    });
    assert_eq!(drops(&counter), 4);
    assert_consistent(&sparse_set);
    for index in [0, 2, 4, 6] {
        assert_eq!(values_of(&sparse_set, id(index)), [index as u32, 10]);
    }

    sparse_set.private_retain(now(), |entity, component| {
        assert_eq!(component.values[0] as u64, entity.index());
        entity == id(4)
    });
    assert_eq!(drops(&counter), 7);
    assert_consistent(&sparse_set);
    assert_eq!(values_of(&sparse_set, id(4)), [4, 10]);
}

/// Asserts that a `retain_mut` callback unwinding partway through a grouped storage leaves it
/// consistent: the components it already rejected are gone and dropped once, the others are all
/// still reachable.
#[cfg(feature = "std")]
#[test]
fn grouped_retain_panic() {
    let counter = counter();
    let mut sparse_set = grouped_items(7, &counter);
    let mut visited = 0;

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        sparse_set.private_retain_mut(now(), |_, _| {
            visited += 1;
            if visited <= 2 {
                return false;
            }
            panic!("retain callback panicked");
        });
    }));
    assert!(result.is_err());
    assert_eq!(drops(&counter), 2);
    assert_eq!(sparse_set.len(), 5);
    assert_consistent(&sparse_set);
    for &entity in &sparse_set.dense {
        assert_eq!(values_of(&sparse_set, entity), [entity.index() as u32]);
    }

    drop(sparse_set);
    assert_eq!(drops(&counter), 7);
}

/// Asserts that a removal callback unwinding, after the component left a grouped range, drops
/// that component exactly once while the storage stays consistent.
#[cfg(feature = "std")]
#[test]
fn grouped_removal_callback_panic() {
    let counter = counter();
    let mut sparse_set = grouped_items(7, &counter);
    sparse_set.on_removal(|_, _| panic!("removal callback panicked"));

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = sparse_set.dyn_remove(id(1), now());
    }));
    assert!(result.is_err());
    assert_eq!(drops(&counter), 1);
    assert_eq!(sparse_set.len(), 6);
    assert_consistent(&sparse_set);
    for &entity in &sparse_set.dense {
        assert_eq!(values_of(&sparse_set, entity), [entity.index() as u32]);
    }

    let _ = sparse_set.take_on_removal();
    drop(sparse_set);
    assert_eq!(drops(&counter), 7);
}

/// Asserts that removing over-aligned components from a grouped storage moves them between rows
/// with every access aligned, each component keeping its value.
#[test]
fn grouped_over_aligned() {
    let mut sparse_set = SparseSet::new();
    for index in 0..6 {
        sparse_set
            .insert(id(index), Aligned(index as u64), now())
            .assert_inserted();
    }
    group(&mut sparse_set, &[(3, 1), (2, -1)], 1);

    for index in [1, 3] {
        let removed = sparse_set.dyn_remove(id(index), now()).unwrap();
        assert_eq!(removed.0, index as u64);
    }
    assert_consistent(&sparse_set);
    for index in [0, 2, 4, 5] {
        let moved = sparse_set.private_get(id(index)).unwrap();
        assert_eq!(moved.0, index as u64);
        assert_eq!(core::ptr::from_ref(moved) as usize % 64, 0);
    }
}

/// Asserts that zero-sized components leave each region of a grouped storage.
#[test]
fn grouped_zero_sized() {
    let mut sparse_set = SparseSet::new();
    for index in 0..7 {
        sparse_set.insert(id(index), Zst, now()).assert_inserted();
    }
    group(&mut sparse_set, &[(3, 1), (2, -1)], 1);

    for index in [6, 5, 0, 3] {
        assert!(sparse_set.dyn_remove(id(index), now()).is_some());
        assert_consistent(&sparse_set);
    }
    assert_eq!(sparse_set.len(), 3);
}

/// Asserts that replacing older generations from a range, the ungrouped and the unevaluated
/// region drops each old component exactly once, right away, while the new components and every
/// other one stay reachable with their values.
#[test]
fn grouped_replacement() {
    let counter = counter();
    let mut sparse_set = grouped_items(7, &counter);

    for (dropped, index) in [1, 4, 5, 6].into_iter().enumerate() {
        let newer = EntityId::new_from_index_and_gen(index as u64, 1);

        let result = sparse_set.insert(newer, item(10 + index as u32, &counter), now());

        assert!(matches!(result, InsertionResult::OtherComponentOverride));
        assert_eq!(drops(&counter), dropped + 1);
        assert_eq!(values_of(&sparse_set, newer), [10 + index as u32]);
        assert!(sparse_set.private_get(id(index)).is_none());
        assert_consistent(&sparse_set);
    }

    for index in [0, 2, 3] {
        assert_eq!(values_of(&sparse_set, id(index)), [index as u32]);
    }
    drop(sparse_set);
    assert_eq!(drops(&counter), 11);
}

/// Asserts that an insertion callback unwinding while replacing a grouped component leaves the
/// storage as it was: the old component is still there, the new value is dropped exactly once.
#[cfg(feature = "std")]
#[test]
fn grouped_replacement_insertion_panic() {
    let counter = counter();
    let mut sparse_set = grouped_items(7, &counter);
    sparse_set.on_insertion(|_, _| panic!("insertion callback panicked"));
    let dense = sparse_set.dense.clone();
    let newer = EntityId::new_from_index_and_gen(1, 1);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = sparse_set.insert(newer, item(10, &counter), now());
    }));

    assert!(result.is_err());
    assert_eq!(drops(&counter), 1);
    assert_eq!(sparse_set.dense, dense);
    assert_eq!(sparse_set.groups.ranges[0].trust, 3);
    assert_eq!(values_of(&sparse_set, id(1)), [1]);
    assert!(sparse_set.private_get(newer).is_none());
    assert_consistent(&sparse_set);

    let _ = sparse_set.take_on_insertion();
    drop(sparse_set);
    assert_eq!(drops(&counter), 8);
}

/// Component whose drop panics when asked to.
struct Fragile {
    panics: bool,
}

impl Drop for Fragile {
    fn drop(&mut self) {
        if self.panics {
            panic!("fragile component dropped");
        }
    }
}

impl Component for Fragile {
    type Tracking = track::Untracked;
}

/// Asserts that an old component panicking while it's dropped by a replacement only does so once
/// the storage is consistent: the new component is reachable, the old entity isn't.
#[cfg(feature = "std")]
#[test]
fn grouped_replacement_drop_panic() {
    let mut sparse_set = SparseSet::new();
    for index in 0..5 {
        let fragile = Fragile { panics: index == 1 };
        sparse_set
            .insert(id(index), fragile, now())
            .assert_inserted();
    }
    group(&mut sparse_set, &[(3, 1), (1, 0)], 1);
    let newer = EntityId::new_from_index_and_gen(1, 1);

    let result = std::panic::catch_unwind(core::panic::AssertUnwindSafe(|| {
        let _ = sparse_set.insert(newer, Fragile { panics: false }, now());
    }));

    assert!(result.is_err());
    assert!(sparse_set.private_get(id(1)).is_none());
    assert!(!sparse_set.private_get(newer).unwrap().panics);
    assert_eq!(sparse_set.len(), 5);
    assert_consistent(&sparse_set);
}
