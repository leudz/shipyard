use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.add_entity(());
    let entity1 = world.add_entity((USIZE(0), U32(1)));

    let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
    assert_eq!((&usizes, &u32s).get(entity1), Ok((&USIZE(0), &U32(1))));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity = world.add_entity((USIZE(0),));

    let usizes = world.borrow::<View<USIZE, track::All>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], USIZE(0));
}

#[test]
fn cleared_update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity1 = world.add_entity((USIZE(1usize),));

    world.run(|usizes: ViewMut<USIZE, track::All>| {
        usizes.clear_all_inserted();
    });

    world.run(|usizes: View<USIZE, track::All>| {
        assert_eq!(usizes.inserted().iter().count(), 0);
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|usizes: View<USIZE, track::All>| {
        assert_eq!(usizes.inserted().iter().count(), 1);
        assert_eq!(*usizes.get(entity1).unwrap(), USIZE(1));
        assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
    });
}

#[test]
fn modified_update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();
    world.track_all::<USIZE>();

    let entity1 = world.add_entity((USIZE(1),));

    world.run(|usizes: ViewMut<USIZE, track::All>| {
        usizes.clear_all_inserted_and_modified();
    });

    let entity2 = world.add_entity((USIZE(2usize),));

    world.run(|mut usizes: ViewMut<USIZE, track::All>| {
        usizes[entity1] = USIZE(3);
        assert_eq!(usizes.inserted().iter().count(), 1);
        assert_eq!(*usizes.get(entity1).unwrap(), USIZE(3));
        assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
    });
}
#[test]
fn bulk_single() {
    let mut world = World::new();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U32(i),)))
        .collect::<Vec<_>>();

    let u32s = world.borrow::<View<U32>>().unwrap();
    let mut iter = u32s.iter();
    assert_eq!(iter.next(), Some(&U32(0)));
    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), Some(&U32(2)));
    assert_eq!(iter.next(), Some(&U32(3)));
    assert_eq!(iter.next(), Some(&U32(4)));
    assert_eq!(iter.next(), None);

    let mut iter = u32s.iter().ids().zip(entities);
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);
}

#[test]
fn bulk() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (U32(i), USIZE(i as usize))))
        .collect::<Vec<_>>();

    let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    let mut iter = (&u32s, &usizes).iter();
    assert_eq!(iter.next(), Some((&U32(0), &USIZE(0))));
    assert_eq!(iter.next(), Some((&U32(1), &USIZE(1))));
    assert_eq!(iter.next(), Some((&U32(2), &USIZE(2))));
    assert_eq!(iter.next(), Some((&U32(3), &USIZE(3))));
    assert_eq!(iter.next(), Some((&U32(4), &USIZE(4))));
    assert_eq!(iter.next(), None);

    let mut iter = u32s.iter().ids().zip(entities.clone());
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);

    let mut iter = usizes.iter().ids().zip(entities);
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next().map(|(left, right)| left == right), Some(true));
    assert_eq!(iter.next(), None);

    drop((u32s, usizes));

    world.bulk_add_entity((0..5).map(|i| (U32(i), USIZE(i as usize))));

    let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    assert_eq!(u32s.len(), 10);
    assert_eq!(usizes.len(), 10);
}

/// Asserts that bulk inserting empty entities through a filter appends fresh IDs and leaves the
/// vacant slot available for the next individual insertion.
#[test]
fn bulk_fresh() {
    let mut world = World::new();
    let deleted = world.add_entity(());
    world.add_entity(());
    assert!(world.delete_entity(deleted));

    let input = [(), ()].into_iter().filter(|_| true);
    let ids = world.bulk_add_entity(input);
    let expected = [
        EntityId::new_from_index_and_gen(2, 0),
        EntityId::new_from_index_and_gen(3, 0),
    ];
    assert_eq!(ids.collect::<Vec<_>>(), expected);

    assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(0, 1));
}

/// Asserts that bulk adding entities with multiple components marks every component as inserted
/// in each storage.
#[test]
fn bulk_inserted() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();
    world.track_insertion::<(U32, USIZE)>();

    world.bulk_add_entity((0..5).map(|i| (U32(i), USIZE(i as usize))));

    let (u32s, usizes) = world
        .borrow::<(View<U32, track::Insertion>, View<USIZE, track::Insertion>)>()
        .unwrap();
    assert_eq!(u32s.inserted().iter().count(), 5);
    assert_eq!(usizes.inserted().iter().count(), 5);
}

/// Asserts that bulk adding no entity with a single component, into an empty storage and into a
/// non-empty one, adds nothing and leaves the existing component reachable.
#[test]
fn bulk_single_empty() {
    let mut world = World::new();

    assert_eq!(world.bulk_add_entity(std::iter::empty::<U32>()).count(), 0);
    let entity = world.add_entity(U32(0));
    assert_eq!(world.bulk_add_entity(std::iter::empty::<U32>()).count(), 0);

    let u32s = world.borrow::<View<U32>>().unwrap();
    assert_eq!(u32s.len(), 1);
    assert_eq!(u32s.get(entity), Ok(&U32(0)));
}

/// Asserts that bulk adding no entity with multiple components, into empty storages and into
/// non-empty ones, adds nothing and leaves the existing components reachable.
#[test]
fn bulk_empty() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    assert_eq!(
        world
            .bulk_add_entity(std::iter::empty::<(U32, USIZE)>())
            .count(),
        0
    );
    let entity = world.add_entity((U32(0), USIZE(1)));
    assert_eq!(
        world
            .bulk_add_entity(std::iter::empty::<(U32, USIZE)>())
            .count(),
        0
    );

    let (u32s, usizes) = world.borrow::<(View<U32>, View<USIZE>)>().unwrap();
    assert_eq!((u32s.len(), usizes.len()), (1, 1));
    assert_eq!(u32s.get(entity), Ok(&U32(0)));
    assert_eq!(usizes.get(entity), Ok(&USIZE(1)));
}

/// Checks that the storage's rows agree across component slices, iteration, lookups, and tracking.
fn assert_bulk_rows<T: Component + PartialEq + std::fmt::Debug>(
    view: &View<'_, T, track::All>,
    expected: &[(EntityId, &T)],
    modified: &[EntityId],
) {
    assert_eq!(view.len(), expected.len());
    assert_eq!(view.as_slice().len(), expected.len());
    assert_eq!(view.iter().with_id().collect::<Vec<_>>(), expected);
    for (index, &(entity, value)) in expected.iter().enumerate() {
        assert_eq!(&view.as_slice()[index], value);
        assert_eq!(view.get(entity), Ok(value));
        assert!(view.is_inserted(entity));
    }
    assert_eq!(view.modified().iter().ids().collect::<Vec<_>>(), modified);
    assert_eq!(view.removed().next(), None);
    assert_eq!(view.deleted().next(), None);
}

/// Asserts that unwinding from a bare-component source preserves existing rows and tracking,
/// and that retrying returns fresh, contiguous IDs for exactly the successful input.
#[test]
fn bulk_single_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut world = World::new();
    world.track_all::<U32>();
    let existing = world.add_entity(U32(1));
    let recycled = world.add_entity(());
    world.delete_entity(recycled);
    world.get::<&mut U32>(existing).unwrap().0 = 2;

    let result = catch_unwind(AssertUnwindSafe(|| {
        world.bulk_add_entity((0..5).map(|value| {
            assert_ne!(value, 3, "bulk source panicked");
            U32(value + 10)
        }));
    }));
    assert!(result.is_err());

    {
        let view = world.borrow::<View<'_, U32, track::All>>().unwrap();
        assert_bulk_rows(&view, &[(existing, &U32(2))], &[existing]);
    }

    let expected = [
        EntityId::new_from_index_and_gen(2, 0),
        EntityId::new_from_index_and_gen(3, 0),
    ];
    let inserted = world.bulk_add_entity([U32(20), U32(21)]);
    assert_eq!(inserted.as_slice(), &expected);
    assert_eq!(inserted.collect::<Vec<_>>(), expected);

    let view = world.borrow::<View<'_, U32, track::All>>().unwrap();
    assert_bulk_rows(
        &view,
        &[
            (existing, &U32(2)),
            (expected[0], &U32(20)),
            (expected[1], &U32(21)),
        ],
        &[existing],
    );
}

/// Asserts that a panicking tuple source restores grouped storages to their distinct initial
/// lengths, and that the next batch appends matching rows for exactly its returned IDs.
#[test]
fn bulk_tuple_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[derive(Debug, PartialEq, Eq)]
    struct Other(u32);

    impl Component for Other {
        type Tracking = track::Untracked;
    }

    let mut world = World::builder().with_group::<(U32, Other)>().build();
    world.track_all::<(U32, Other)>();
    let pair = world.add_entity((U32(1), Other(10)));
    let single = world.add_entity(U32(2));
    world.regroup();

    let result = catch_unwind(AssertUnwindSafe(|| {
        world.bulk_add_entity((0..5).map(|value| {
            assert_ne!(value, 3, "bulk source panicked");
            (U32(value + 3), Other(value + 30))
        }));
    }));
    assert!(result.is_err());

    {
        let (numbers, others) = world
            .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
            .unwrap();
        assert_bulk_rows(&numbers, &[(pair, &U32(1)), (single, &U32(2))], &[]);
        assert_bulk_rows(&others, &[(pair, &Other(10))], &[]);
    }

    let expected = [
        EntityId::new_from_index_and_gen(2, 0),
        EntityId::new_from_index_and_gen(3, 0),
    ];
    let inserted = world.bulk_add_entity([(U32(5), Other(50)), (U32(6), Other(60))]);
    assert_eq!(inserted.as_slice(), &expected);
    assert_eq!(inserted.collect::<Vec<_>>(), expected);

    let (numbers, others) = world
        .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
        .unwrap();
    assert_bulk_rows(
        &numbers,
        &[
            (pair, &U32(1)),
            (single, &U32(2)),
            (expected[0], &U32(5)),
            (expected[1], &U32(6)),
        ],
        &[],
    );
    assert_bulk_rows(
        &others,
        &[
            (pair, &Other(10)),
            (expected[0], &Other(50)),
            (expected[1], &Other(60)),
        ],
        &[],
    );
}

mod bulk_callbacks {
    use super::{U32, assert_bulk_rows};
    use shipyard::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, PartialEq, Eq)]
    struct Other(u32);

    impl Component for Other {
        type Tracking = track::Untracked;
    }

    /// Records each insertion's entity and value.
    fn record_insertions<T: Component>(
        view: &mut ViewMut<'_, T>,
        value: fn(&T) -> u32,
    ) -> Arc<Mutex<Vec<(EntityId, u32)>>> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::clone(&log);
        view.on_insertion(move |entity, component| {
            calls.lock().unwrap().push((entity, value(component)));
        });
        log
    }

    /// Asserts that bare components and one-tuples report the stored values with the fresh IDs
    /// returned by the batch, even when a recyclable ID is available.
    #[test]
    fn single() {
        for one_tuple in [false, true] {
            let mut world = World::new();
            world.track_all::<U32>();
            let recycled = world.add_entity(());
            assert!(world.delete_entity(recycled));
            let calls =
                record_insertions(&mut world.borrow::<ViewMut<'_, U32>>().unwrap(), |v| v.0);

            let ids = if one_tuple {
                world.bulk_add_entity([(U32(10),), (U32(20),)])
            } else {
                world.bulk_add_entity([U32(10), U32(20)])
            };
            let first = EntityId::new_from_index_and_gen(1, 0);
            let second = EntityId::new_from_index_and_gen(2, 0);
            assert_eq!(ids.as_slice(), &[first, second]);
            assert_eq!(ids.collect::<Vec<_>>(), [first, second]);
            assert_eq!(*calls.lock().unwrap(), [(first, 10), (second, 20)]);
            assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(0, 1));

            let view = world.borrow::<View<'_, U32, track::All>>().unwrap();
            assert_bulk_rows(&view, &[(first, &U32(10)), (second, &U32(20))], &[]);
        }
    }

    /// Asserts that each storage in a tuple receives the right ID and value for every row.
    #[test]
    fn tuple() {
        let mut world = World::new();
        world.track_all::<(U32, Other)>();
        let (number_calls, other_calls) = {
            let (mut numbers, mut others) = world
                .borrow::<(ViewMut<'_, U32>, ViewMut<'_, Other>)>()
                .unwrap();
            (
                record_insertions(&mut numbers, |v| v.0),
                record_insertions(&mut others, |v| v.0),
            )
        };

        let ids = world.bulk_add_entity([(U32(1), Other(10)), (U32(2), Other(20))]);
        let first = EntityId::new_from_index_and_gen(0, 0);
        let second = EntityId::new_from_index_and_gen(1, 0);
        assert_eq!(ids.as_slice(), &[first, second]);
        assert_eq!(ids.collect::<Vec<_>>(), [first, second]);
        assert_eq!(*number_calls.lock().unwrap(), [(first, 1), (second, 2)]);
        assert_eq!(*other_calls.lock().unwrap(), [(first, 10), (second, 20)]);

        let (numbers, others) = world
            .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
            .unwrap();
        assert_bulk_rows(&numbers, &[(first, &U32(1)), (second, &U32(2))], &[]);
        assert_bulk_rows(&others, &[(first, &Other(10)), (second, &Other(20))], &[]);
    }

    /// Asserts that empty batches leave callbacks and entity allocation untouched.
    #[test]
    fn empty() {
        let mut world = World::new();
        let calls = record_insertions(&mut world.borrow::<ViewMut<'_, U32>>().unwrap(), |v| v.0);

        assert!(
            world
                .bulk_add_entity(std::iter::empty::<U32>())
                .as_slice()
                .is_empty()
        );
        assert!(
            world
                .bulk_add_entity(std::iter::empty::<(U32,)>())
                .as_slice()
                .is_empty()
        );
        assert!(
            world
                .bulk_add_entity(std::iter::empty::<(Other, U32)>())
                .as_slice()
                .is_empty()
        );
        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(0, 0));
    }

    /// Asserts that when a later storage's callback panics, previously inserted rows and
    /// tracking stay coherent and a subsequent batch inserts into both storages.
    #[test]
    fn callback_unwind() {
        let mut world = World::builder().with_group::<(U32, Other)>().build();
        world.track_all::<(U32, Other)>();
        let existing = world.add_entity((U32(0), Other(0)));
        world.regroup();
        world
            .borrow::<ViewMut<'_, Other>>()
            .unwrap()
            .on_insertion(|_, value| {
                assert_ne!(value.0, 99, "insertion callback panicked");
            });

        let result = catch_unwind(AssertUnwindSafe(|| {
            world.bulk_add_entity([
                (U32(1), Other(10)),
                (U32(2), Other(99)),
                (U32(3), Other(30)),
            ]);
        }));
        assert!(result.is_err());
        let complete = EntityId::new_from_index_and_gen(1, 0);
        let partial = EntityId::new_from_index_and_gen(2, 0);
        let next = EntityId::new_from_index_and_gen(3, 0);
        assert!(world.is_entity_alive(complete));
        assert!(world.is_entity_alive(partial));
        assert!(!world.is_entity_alive(next));
        {
            let (numbers, others) = world
                .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
                .unwrap();
            assert_bulk_rows(
                &numbers,
                &[(existing, &U32(0)), (complete, &U32(1)), (partial, &U32(2))],
                &[],
            );
            assert_bulk_rows(
                &others,
                &[(existing, &Other(0)), (complete, &Other(10))],
                &[],
            );
            assert!(!others.contains(partial));
        }

        assert_eq!(
            world.bulk_add_entity([(U32(4), Other(40))]).as_slice(),
            &[next]
        );
        let (numbers, others) = world
            .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
            .unwrap();
        assert_bulk_rows(
            &numbers,
            &[
                (existing, &U32(0)),
                (complete, &U32(1)),
                (partial, &U32(2)),
                (next, &U32(4)),
            ],
            &[],
        );
        assert_bulk_rows(
            &others,
            &[
                (existing, &Other(0)),
                (complete, &Other(10)),
                (next, &Other(40)),
            ],
            &[],
        );
        assert!(!others.contains(partial));
    }

    /// Asserts that a panicking source leaves the rows and notifications for items
    /// already inserted through the callback path.
    #[test]
    fn source_unwind() {
        let mut world = World::new();
        world.track_all::<(U32, Other)>();
        let calls = record_insertions(&mut world.borrow::<ViewMut<'_, U32>>().unwrap(), |v| v.0);

        let result = catch_unwind(AssertUnwindSafe(|| {
            world.bulk_add_entity((0..4).map(|i| {
                assert_ne!(i, 2, "bulk source panicked");
                (U32(i), Other(i))
            }));
        }));
        assert!(result.is_err());
        let first = EntityId::new_from_index_and_gen(0, 0);
        let second = EntityId::new_from_index_and_gen(1, 0);
        assert_eq!(*calls.lock().unwrap(), [(first, 0), (second, 1)]);
        assert!(world.is_entity_alive(first));
        assert!(world.is_entity_alive(second));

        let (numbers, others) = world
            .borrow::<(View<'_, U32, track::All>, View<'_, Other, track::All>)>()
            .unwrap();
        assert_bulk_rows(&numbers, &[(first, &U32(0)), (second, &U32(1))], &[]);
        assert_bulk_rows(&others, &[(first, &Other(0)), (second, &Other(1))], &[]);
        drop((numbers, others));

        assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(2, 0));
    }

    /// Asserts that the entity-view bulk API still notifies the observer once per row.
    #[test]
    fn entity_view() {
        let world = World::new();
        let (mut entities, mut values) = world
            .borrow::<(EntitiesViewMut<'_>, ViewMut<'_, U32>)>()
            .unwrap();
        let calls = record_insertions(&mut values, |v| v.0);

        let ids = entities.bulk_add_entity(&mut values, [U32(5), U32(6)]);
        let first = EntityId::new_from_index_and_gen(0, 0);
        let second = EntityId::new_from_index_and_gen(1, 0);
        assert_eq!(ids.as_slice(), &[first, second]);
        assert_eq!(*calls.lock().unwrap(), [(first, 5), (second, 6)]);
    }
}
