use super::*;
use crate::r#mut::Mut;
use crate::{Component, Get, IntoIter, View, ViewMut, World, track};

#[derive(Clone)]
struct Tracked(usize);
impl Component for Tracked {
    type Tracking = track::All;
}
struct Marker;
impl Component for Marker {
    type Tracking = track::Untracked;
}

fn check(world: &World) {
    let view = world.borrow::<View<'_, Tracked>>().unwrap();
    for (chunks, timestamps, modified) in [
        (&view.insertion_chunks, &view.insertion_data, false),
        (
            &view.modification_summary.chunks,
            &view.modification_data,
            true,
        ),
    ] {
        assert_eq!(chunks.len(), timestamps.len().div_ceil(TRACKING_CHUNK_SIZE));
        for (chunk, timestamps) in chunks.iter().zip(timestamps.chunks(TRACKING_CHUNK_SIZE)) {
            assert!(timestamps.iter().all(|t| t.get()
                <= if modified {
                    view.modification_summary.upper(chunk)
                } else {
                    chunk.0.load(Ordering::Relaxed)
                }));
        }
    }
}

#[test]
fn growth_does_not_inherit_whole_marks() {
    for bulk in [false, true] {
        let mut world = World::new_for_test();
        world.bulk_add_entity((0..64).map(|i| (Tracked(i),)));
        {
            let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
            assert_eq!((&mut view).iter().count(), 64);
        }
        if bulk {
            world.bulk_add_entity((64..128).map(|i| (Tracked(i),)));
        } else {
            world.add_entity(Tracked(64));
        }
        let view = world.borrow::<View<'_, Tracked>>().unwrap();
        assert!(
            view.modification_summary
                .changed(0, TrackingTimestamp::origin())
        );
        assert!(
            !view
                .modification_summary
                .changed(1, TrackingTimestamp::origin())
        );
        assert_eq!(view.modified().iter().count(), 0);
    }
}

#[test]
fn clear_and_drain_reuse_modification_chunks() {
    for drain in [false, true] {
        let mut world = World::new_for_test();
        world.add_entity(Tracked(0));
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let capacity = view.modification_summary.capacity();
        assert_eq!((&mut view).iter().count(), 1);
        assert!(
            view.modification_summary
                .all_changed(TrackingTimestamp::origin())
        );

        if drain {
            assert_eq!(view.drain().count(), 1);
        } else {
            view.clear();
        }

        assert_eq!(view.modification_summary.len(), 0);
        assert_eq!(view.modification_summary.capacity(), capacity);
        assert!(
            !view
                .modification_summary
                .all_changed(TrackingTimestamp::origin())
        );
    }
}

#[test]
fn missing_modification_chunk_keeps_exact_matches() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = world
        .bulk_add_entity((0..TRACKING_CHUNK_SIZE + 2).map(|i| (Tracked(i),)))
        .collect();
    let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
    let id = ids[TRACKING_CHUNK_SIZE];
    view[id].0 += 1;
    view.modification_summary.chunks.pop().unwrap();
    assert_eq!(view.modified().iter().ids().collect::<Vec<_>>(), [id]);
}

#[test]
fn updates_keep_the_latest_timestamp() {
    let mut chunk = ChunkTimestamp::default();
    chunk.mark_mut(TrackingTimestamp::new(10));
    chunk.mark(TrackingTimestamp::new(5));
    assert_eq!(chunk.0.load(Ordering::Relaxed), 10);
    chunk.mark(TrackingTimestamp::new(11));
    assert!(chunk.changed_since(TrackingTimestamp::new(10)));
}

#[test]
fn mutation_paths_and_read_only_access() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..259).map(|i| world.add_entity((Tracked(i),))).collect();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        assert_eq!((&mut view).get(ids[0]).unwrap().0, 0);
        assert!(
            view.modification_summary
                .chunks
                .iter()
                .all(|c| c.0.load(Ordering::Relaxed) == 0)
        );
        assert_eq!(
            (&mut view).iter().map(|t| t.0).sum::<usize>(),
            (0..259).sum::<usize>()
        );
        // All variants preserve exact item flags after a read-only traversal.
        assert_eq!(view.modified().iter().count(), 0);
        let mut item = (&mut view).get(ids[0]).unwrap();
        item.0 += 1;
        *Mut::map(item, |t| &mut t.0).as_mut() += 1;
        view[ids[64]].0 += 1;
        view.apply_mut(ids[128], ids[192], |a, b| {
            a.0 += 1;
            b.0 += 1;
        });
        view.retain_mut(|_, mut t| {
            if t.0 == 258 {
                t.0 += 1;
            }
            true
        });
    }
    {
        let mut item = world.get::<&mut Tracked>(ids[256]).unwrap();
        item.0 += 1;
        *crate::get_component::RefMut::map(item, |t| &mut t.0).as_mut() += 1;
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        6
    );
    let view = world.borrow::<View<'_, Tracked>>().unwrap();
    assert_eq!(view[ids[0]].0, 2);
    assert_eq!(view[ids[256]].0, 258);
}

#[test]
fn enabling_tracking_fills_existing_chunks() {
    let mut storage = crate::sparse_set::SparseSet::<Marker>::new();
    for i in 0..129 {
        let _ = storage.insert(
            crate::EntityId::new_from_index_and_gen(i, 0),
            Marker,
            TrackingTimestamp::new(i),
        );
    }
    storage.track_all();
    assert_eq!(storage.insertion_chunks.len(), 3);
    assert_eq!(storage.modification_summary.chunks.len(), 3);
}

#[test]
fn moving_rows_and_bulk_insertion_preserve_summaries() {
    let mut world = World::builder_for_test()
        .with_group::<(Marker, Tracked)>()
        .build();
    let ids: Vec<_> = world
        .bulk_add_entity((0..259).map(|i| (Tracked(i),)))
        .collect();
    world.borrow::<ViewMut<'_, Tracked>>().unwrap()[ids[258]].0 += 1;
    for &id in ids.iter().step_by(3) {
        world.add_component(id, Marker);
    }
    world.regroup();
    check(&world);
    world.delete_entity(ids[0]);
    check(&world);
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .sort_unstable_by(|a, b| b.0.cmp(&a.0));
    check(&world);
    #[cfg(feature = "std")]
    {
        world.register_clone::<crate::sparse_set::SparseSet<Tracked>>();
        check(&world.clone());
    }
    world.borrow::<ViewMut<'_, Tracked>>().unwrap().clear();
    check(&world);
}

#[cfg(feature = "parallel")]
#[test]
fn retained_mut_items_can_write_in_parallel() {
    use rayon::prelude::*;
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..259).map(|i| (Tracked(i),)));
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let items = (&mut view).iter().fold(Vec::new(), |mut items, t| {
            assert!(t.chunk.is_none());
            items.push(t);
            items
        });
        items.into_par_iter().for_each(|mut t| t.0 += 1);
    }
    check(&world);
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        (&mut view).par_iter().for_each(|mut t| t.0 += 1);
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        259
    );
    let chunk = ChunkTimestamp::default();
    let current = TrackingTimestamp::new(1);
    (0..64).into_par_iter().for_each(|_| chunk.mark(current));
    assert_eq!(chunk.0.load(Ordering::Relaxed), current.get());
}

#[test]
fn folding_only_marks_accessed_components() {
    for offset in [0, 63] {
        let mut world = World::new_for_test();
        let ids: Vec<_> = world
            .bulk_add_entity((0..130).map(|i| (Tracked(i),)))
            .collect();
        {
            let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
            (&mut view).iter().for_each(|mut t| {
                if t.0 % TRACKING_CHUNK_SIZE == offset {
                    t.0 += 1;
                }
            });
        }
        check(&world);
        let view = world.borrow::<View<'_, Tracked>>().unwrap();
        assert_eq!(
            view.modified().iter().ids().collect::<Vec<_>>(),
            ids.iter()
                .skip(offset)
                .step_by(TRACKING_CHUNK_SIZE)
                .copied()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn retained_items_without_chunk_references_still_track() {
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..130).map(|i| (Tracked(i),)));
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let items = (&mut view).iter().fold(Vec::new(), |mut items, mut t| {
            if t.0 % TRACKING_CHUNK_SIZE == 0 {
                t.0 += 1;
            } else {
                assert!(t.chunk.is_none());
            }
            items.push(t);
            items
        });
        for t in items.into_iter().rev() {
            *Mut::map(t, |t| &mut t.0).as_mut() += 1;
        }
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        130
    );
}

#[test]
fn folding_uses_absolute_chunk_boundaries() {
    use crate::iter::Shiperator;
    use crate::sparse_set::FullRawWindowMut;
    for (start, len, skip) in [(5, 60, 0), (31, 34, 1), (63, 2, 0)] {
        let mut world = World::new_for_test();
        let ids: Vec<_> = world
            .bulk_add_entity((0..130).map(|i| (Tracked(i),)))
            .collect();
        {
            let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
            let mut window = FullRawWindowMut::new(&mut view);
            let mut cursor = window.entity_iter().cursor.segment(start, len);
            for _ in 0..skip {
                cursor.advance();
            }
            // SAFE: This cursor's entities match the installed storage range; each is visited once.
            let visited = unsafe {
                window.set_segment_start(start);
                window.fold_segment::<true, _, _>(&mut cursor, Vec::new(), |mut ids, id, mut t| {
                    t.0 += 1;
                    ids.push(id.unwrap());
                    ids
                })
            };
            assert!(cursor.is_empty());
            assert_eq!(visited, ids[start + skip..start + len]);
        }
        check(&world);
        assert_eq!(
            world
                .borrow::<View<'_, Tracked>>()
                .unwrap()
                .modified()
                .iter()
                .count(),
            len - skip
        );
    }
}

#[cfg(feature = "std")]
#[test]
fn a_panicking_fold_keeps_visited_modifications() {
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..130).map(|i| (Tracked(i),)));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        (&mut view).iter().for_each(|mut t| {
            let index = t.0;
            t.0 += 1;
            assert!(index < 65, "stop during the second chunk");
        });
    }));
    assert!(result.is_err());
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        66
    );
}

#[test]
fn next_prepares_before_yielding_and_keeps_exact_flags() {
    for count in [0, 1, 130] {
        let mut world = World::new_for_test();
        let ids: Vec<_> = world
            .bulk_add_entity((0..count).map(|i| (Tracked(i),)))
            .collect();
        {
            let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
            let mut iter = (&mut view).iter();
            let held = iter.next();
            for t in iter.by_ref().take(64) {
                assert!(t.chunk.is_none());
            }
            drop(iter);
            if let Some(mut t) = held {
                assert!(t.chunk.is_none());
                t.0 += 1;
            }
        }
        check(&world);
        let view = world.borrow::<View<'_, Tracked>>().unwrap();
        assert_eq!(
            view.modified().iter().ids().collect::<Vec<_>>(),
            ids[..count.min(1)]
        );
    }
}

#[cfg(feature = "parallel")]
#[test]
fn split_next_keeps_prepared_chunks() {
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..130).map(|i| (Tracked(i),)));
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let mut iter = (&mut view).iter();
        let first = iter.next().unwrap();
        let (left, right) = iter.split_current_segment_at(63);
        for mut t in right.chain(left).chain(core::iter::once(first)) {
            assert!(t.chunk.is_none());
            t.0 += 1;
        }
    }
    check(&world);
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        130
    );
}

#[test]
fn mixed_sailors_keep_per_item_tracking() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = world
        .bulk_add_entity((0..130).map(|i| (Tracked(i),)))
        .collect();
    world.add_component(ids[129], Marker);
    {
        let (mut view, marker) = world
            .borrow::<(ViewMut<'_, Tracked>, View<'_, Marker>)>()
            .unwrap();
        let mut iter = (&mut view, &marker).iter();
        let (mut t, _) = iter.next().unwrap();
        assert!(t.chunk.is_some());
        drop(iter);
        t.0 += 1;
    }
    check(&world);
    let view = world.borrow::<View<'_, Tracked>>().unwrap();
    assert_eq!(
        view.modification_summary.chunks[0]
            .0
            .load(Ordering::Relaxed),
        0
    );
    assert_eq!(view.modified().iter().ids().collect::<Vec<_>>(), [ids[129]]);
}

#[test]
fn preparing_an_offset_segment_marks_only_its_chunks() {
    use crate::iter::plan::NodeState;
    use crate::iter::{SegmentEntry, Shiperator};
    use crate::sparse_set::FullRawWindowMut;
    let mut world = World::new_for_test();
    let ids: Vec<_> = world
        .bulk_add_entity((0..259).map(|i| (Tracked(i),)))
        .collect();
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let mut window = FullRawWindowMut::new(&mut view);
        let row = [NodeState::Window { segment_start: 63 }];
        let mut entry = SegmentEntry::without_coverage(&row);
        // SAFE: Local 1..66 maps to storage 64..129; each component is visited once.
        unsafe {
            window.enter_segment(&mut entry);
            window.prepare_tracking(Some(1..66));
            for index in 64..129 {
                let mut t = window.get_data(index);
                assert!(t.chunk.is_none());
                t.0 += 1;
            }
            window.set_segment_start(0);
        }
        assert!(!window.chunks_prepared);
    }
    check(&world);
    let view = world.borrow::<View<'_, Tracked>>().unwrap();
    assert_eq!(
        view.modification_summary.chunks[0]
            .0
            .load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        view.modification_summary.chunks[3]
            .0
            .load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        view.modified().iter().ids().collect::<Vec<_>>(),
        ids[64..129]
    );
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_tracking_queries_keep_summaries() {
    use rayon::prelude::*;
    let mut world = World::new_for_test();
    world.bulk_add_entity((0..259).map(|i| (Tracked(i),)));
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        for mut t in (&mut view).iter().take(65) {
            t.0 += 1;
        }
    }
    (0..16).into_par_iter().for_each(|_| {
        assert_eq!(
            world
                .borrow::<View<'_, Tracked>>()
                .unwrap()
                .modified()
                .iter()
                .count(),
            65
        );
    });
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        view.modified_mut().par_iter().for_each(|mut t| t.0 += 1);
    }
    check(&world);
}

#[test]
fn summary_query_finds_late_mutations_and_rows_moved_after_invalidation() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = world
        .bulk_add_entity((0..259).map(|i| (Tracked(i),)))
        .collect();
    {
        let mut view = world.borrow::<ViewMut<'_, Tracked>>().unwrap();
        let items: Vec<_> = (&mut view).iter().collect();
        for mut t in items.into_iter().skip(128).take(2) {
            t.0 += 1;
        }
    }
    world.delete_entity(ids[0]);
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .sort_unstable_by(|a, b| b.0.cmp(&a.0));
    let view = world.borrow::<View<'_, Tracked>>().unwrap();
    let mut actual = view.modified().iter().ids().collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, ids[128..130]);
    drop(view);
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        0
    );
    world.borrow::<ViewMut<'_, Tracked>>().unwrap().clear();
    world.bulk_add_entity((0..129).map(|i| (Tracked(i),)));
    assert_eq!(
        world
            .borrow::<View<'_, Tracked>>()
            .unwrap()
            .modified()
            .iter()
            .count(),
        0
    );
    check(&world);
}
