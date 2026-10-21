use super::super::layout::{build, finish};
use super::super::original_row;
use super::*;

/// Every `(entity, value)` of A and B, sorted.
type Values = (Vec<(EntityId, u32)>, Vec<(EntityId, u32)>);

fn values(world: &World) -> Values {
    let collect = |pairs: &mut dyn Iterator<Item = (EntityId, u32)>| sorted_values(pairs.collect());
    let a = world.borrow::<View<'_, A>>().unwrap();
    let b = world.borrow::<View<'_, B>>().unwrap();

    (
        collect(&mut a.iter().with_id().map(|(entity, a)| (entity, a.0))),
        collect(&mut b.iter().with_id().map(|(entity, b)| (entity, b.0))),
    )
}

fn sorted_values(mut values: Vec<(EntityId, u32)>) -> Vec<(EntityId, u32)> {
    values.sort_unstable();
    values
}

/// Regroups `world` with the two-phase path, panicking unless, before any component moves,
/// every storage only reordered its ids, its sparse array still points at their starting rows,
/// the rows outside its envelope kept their id, and every group's members hold the same
/// logical sequence. Afterwards the world has to be clean, a second regroup has nothing to do.
///
/// Returns the number of rows whose id changed in each grouped storage.
fn two_phases(world: &World) -> BTreeMap<StorageId, usize> {
    let mut all_storages = world.all_storages_mut().unwrap();
    let storages = all_storages.storages.get_mut();
    let storage_ids = sorted_storage_ids(storages);

    let frozen = Frozen::new(storages, &storage_ids);
    let entry: Vec<Vec<EntityId>> = frozen.dense.iter().map(|dense| dense.to_vec()).collect();
    drop(frozen);

    let Some(plan) = analyze(storages, &storage_ids) else {
        return BTreeMap::new();
    };
    // SAFE: we only read the storages until `finish` completes them.
    let layouts = unsafe { build(storages, &storage_ids, &plan) };

    let frozen = Frozen::new(storages, &storage_ids);
    let mut changed = BTreeMap::new();
    // The logical sequence of each group, from the first member we see.
    let mut sequences: Vec<(Arc<[StorageId]>, Vec<EntityId>)> = Vec::new();

    for (((membership, dense), entry), layout) in frozen
        .memberships
        .storages
        .iter()
        .zip(&frozen.dense)
        .zip(&entry)
        .zip(&layouts)
    {
        let storage_id = membership.storage_id;
        assert_eq!(layout.storage_id, storage_id);

        assert_eq!(
            sorted(dense.to_vec()),
            sorted(entry.clone()),
            "{storage_id:?}"
        );
        for (row, &entity) in entry.iter().enumerate() {
            assert_eq!(original_row(membership.sparse, entity), Some(row));
        }

        let envelope = layout.envelope.clone().unwrap_or(0..0);
        let mut count = 0;
        for row in 0..dense.len() {
            if dense[row] != entry[row] {
                assert!(envelope.contains(&row), "{storage_id:?} row {row}");
                count += 1;
            }
        }
        changed.insert(storage_id, count);

        for (range, ring) in membership.groups.ranges.iter().zip(&layout.rings) {
            let sequence: Vec<EntityId> = (0..ring.len)
                .map(|logical| dense[ring.physical_of(logical)])
                .collect();

            match sequences
                .iter()
                .find(|(group, _)| Arc::ptr_eq(group, &range.group))
            {
                Some((group, expected)) => assert_eq!(&sequence, expected, "{group:?}"),
                None => sequences.push((range.group.clone(), sequence)),
            }
        }
    }
    drop(frozen);

    // SAFE: `layouts` come from `build` just above, nothing touched the storages since.
    unsafe { finish(storages, &layouts) };

    // A regroup right after this one has nothing to do, it doesn't move anything.
    assert!(analyze(storages, &storage_ids).is_none());

    changed
}

/// Regroups `world` with the two-phase path, then panics unless every group is aligned and fully
/// trusted, every entity sits where its components and the `declared` groups say,
/// and every A and B keeps its value.
///
/// Returns the number of rows whose id changed in each grouped storage.
fn check(world: &World, declared: &[&[StorageId]]) -> BTreeMap<StorageId, usize> {
    let before = values(world);

    let changed = two_phases(world);

    assert_regrouped(world);
    assert_memberships(world, declared);
    assert_eq!(values(world), before);

    changed
}

/// Asserts that removing B from the logical last pair moves no id at all:
/// the pair leaves the range from its end, which becomes the ungrouped region.
#[test]
fn tail() {
    let (mut world, _) = pairs(6);
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    world.remove::<(B,)>(order[5]);

    let changed = check(&world, &[&ab()]);

    assert!(changed.values().all(|&count| count == 0), "{changed:?}");
}

/// Asserts that when B loses and gets back a component, the untouched A, which trusts its whole
/// range, is the reference and keeps every id where it is.
#[test]
fn reinsert() {
    let (mut world, _) = pairs(8);
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    world.remove::<(B,)>(order[1]);
    world.add_component(order[1], B(1));

    let changed = check(&world, &[&ab()]);

    assert_eq!(changed[&id::<A>()], 0);
}

/// Asserts that permanent B removals spread over the range regroup every remaining pair.
#[test]
fn removal() {
    let (mut world, _) = pairs(12);
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    for i in [2, 5, 9] {
        world.remove::<(B,)>(order[i]);
    }

    check(&world, &[&ab()]);
}

/// Asserts that deletions and removals in both members regroup every remaining pair.
#[test]
fn both_members() {
    let (mut world, _) = pairs(12);
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    world.delete_entity(order[1]);
    world.remove::<(A,)>(order[4]);
    world.remove::<(B,)>(order[7]);

    check(&world, &[&ab()]);
}

/// Asserts that a group attached but never regrouped receives every pair.
#[test]
fn attached() {
    let mut world = World::new_for_test();
    for i in 0..5 {
        world.add_entity((A(i), B(i)));
    }
    world.add_entity((A(5),));
    attach::<A, B>(&world);

    check(&world, &[&ab()]);
}

/// Asserts that clearing B and giving it back to some entities regroups only these.
#[test]
fn clear_refill() {
    let (mut world, e) = pairs(6);
    world.borrow::<ViewMut<'_, B>>().unwrap().clear();
    for i in [1, 4] {
        world.add_component(e[i], B(i as u32));
    }

    check(&world, &[&ab()]);
}

/// Asserts that an entity reusing a deleted grouped entity's index is grouped with its own
/// components.
#[test]
fn generation() {
    let (mut world, e) = pairs(6);
    world.delete_entity(e[2]);
    let reused = world.add_entity((A(2), B(2)));
    assert_eq!(reused.index(), e[2].index());

    check(&world, &[&ab()]);
}

/// Asserts the walkthrough: with AB then AC in A, an entity whose B is ungrouped gains A.
/// It's gathered at the start of B's ungrouped region, AB grows by one in both members,
/// and AC slides one slot in A, only its first entity moving to its other end.
#[test]
fn walkthrough() {
    let mut world = World::new_for_test();
    for i in 0..4 {
        world.add_entity((A(i), B(i)));
    }
    for i in 4..7 {
        world.add_entity((A(i), C));
    }
    world.add_entity((A(7),));
    world.add_entity((A(8),));
    world.add_entity((B(9),));
    let e = world.add_entity((B(10),));
    world.group::<(A, B)>().group::<(A, C)>();
    let ac = groups_of::<A>(&world)[1].clone();
    let (ac_before, _) = group_of::<A>(&world, &ac);
    world.add_component(e, A(10));

    let changed = check(&world, &[&ab(), &[id::<A>(), id::<C>()]]);

    // `e` joins AB, AC's first entity moves to its end and an ungrouped A takes `e`'s old row.
    assert_eq!(changed[&id::<A>()], 3);
    assert!(changed[&id::<B>()] <= 2);
    assert_eq!(group_of::<A>(&world, &ac).0, ac_before);
}

/// Asserts that with AB and AC, a pair gaining C leaves AB's kept prefix through gathering,
/// and joins the ABC union with its C.
#[test]
fn gain_leaves_prefix() {
    let (mut world, _) = pairs(6);
    world.group::<(A, C)>();
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    world.add_component(order[1], C);
    world.add_component(order[3], C);

    check(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that with AB and AC, a pair gaining C while the pair at the end of AB's kept prefix
/// gains C too is gathered past it, instead of swapping it back into the prefix.
#[test]
fn gather_edge() {
    let (mut world, _) = pairs(6);
    world.group::<(A, C)>();
    let group = groups_of::<A>(&world)[0].clone();
    let (order, _) = group_of::<A>(&world, &group);
    world.add_component(order[1], C);
    world.add_component(order[5], C);

    check(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that with AB and AC, ungrouped As gaining B, C or both are gathered and grouped.
#[test]
fn promotions() {
    let (mut world, _) = pairs(4);
    world.group::<(A, C)>();
    let lone: Vec<EntityId> = (4..8).map(|i| world.add_entity((A(i),))).collect();
    world.regroup();
    world.add_component(lone[0], B(4));
    world.add_component(lone[2], (B(6), C));
    world.add_component(lone[3], C);

    check(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that with AB, CD, BE and CE, an entity grouped in both AB and CD gaining E
/// joins them in a single ABCDE union.
#[test]
fn joining() {
    let mut world = World::new_for_test();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C, D)))
        .collect();
    world
        .group::<(A, B)>()
        .group::<(C, D)>()
        .group::<(B, E)>()
        .group::<(C, E)>();
    world.add_component(e[2], E);

    let declared: [&[StorageId]; 4] = [
        &ab(),
        &[id::<C>(), id::<D>()],
        &[id::<B>(), id::<E>()],
        &[id::<C>(), id::<E>()],
    ];
    check(&world, &declared);
}

/// Asserts that with AB, BC and CD, an entity of the ABCD union losing D lands in a new ABC union.
#[test]
fn demotion_union() {
    let mut world = World::new_for_test();
    let e: Vec<EntityId> = (0..4)
        .map(|i| world.add_entity((A(i), B(i), C, D)))
        .collect();
    world.group::<(A, B)>().group::<(B, C)>().group::<(C, D)>();
    world.remove::<(D,)>(e[1]);

    let declared: [&[StorageId]; 3] = [&ab(), &[id::<B>(), id::<C>()], &[id::<C>(), id::<D>()]];
    check(&world, &declared);
}

/// Asserts that declaring AC next to AB, invalidated like a declaration does,
/// sends each entity to AB, AC or their union.
#[test]
fn declaration() {
    let (mut world, _) = pairs(4);
    world.add_entity((A(4), B(4), C));
    world.add_entity((A(5), C));
    world.regroup();
    let ac = sorted_ids(vec![id::<A>(), id::<C>()]);
    {
        let mut all_storages = world.all_storages_mut().unwrap();
        crate::all_storages::invalidate_overlaps(all_storages.storages.get_mut(), &ac);
    }
    attach_ids(&world, &ac);

    check(&world, &[&ab(), &ac]);
}

/// Asserts that a `NonSend` member is opened and laid out like any other storage,
/// after a deletion and a new pair.
#[cfg(feature = "thread_local")]
#[test]
fn non_send() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::new_for_test();
    let add_pair = |world: &mut World, i: u32| {
        let entity = world.add_entity((A(i),));
        world
            .borrow::<NonSend<ViewMut<'_, B>>>()
            .unwrap()
            .add_component_unchecked(entity, B(i));
        entity
    };
    let e: Vec<EntityId> = (0..6).map(|i| add_pair(&mut world, i)).collect();
    world.group::<(A, NonSend<B>)>();
    world.delete_entity(e[1]);
    add_pair(&mut world, 6);
    let non_send_values = |world: &World| {
        let b = world.borrow::<NonSend<View<'_, B>>>().unwrap();
        sorted_values(
            b.iter()
                .with_id()
                .map(|(entity, b)| (entity, b.0))
                .collect(),
        )
    };
    let before = non_send_values(&world);

    two_phases(&world);

    assert_regrouped(&world);
    assert_memberships(
        &world,
        &[&[id::<A>(), StorageId::of::<NonSend<SparseSet<B>>>()]],
    );
    assert_eq!(non_send_values(&world), before);
}

/// Returns a world with AB, AC and their ABC union rotated in A, and its entities.
fn rotated() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let e: Vec<EntityId> = (0..24)
        .map(|i| match i % 3 {
            0 => world.add_entity((A(i), B(i))),
            1 => world.add_entity((A(i), C)),
            _ => world.add_entity((A(i), B(i), C)),
        })
        .collect();
    world.group::<(A, B)>().group::<(A, C)>();
    // Promotions shrink AB and AC in A, which rotates the ABC range behind them.
    world.add_component(e[0], C);
    world.add_component(e[4], B(4));
    world.regroup();

    (world, e)
}

/// Asserts that with AB, AC and their ABC union rotated in A, pairs gaining C, ABC entities
/// losing C or B, and an AC pair gaining B all end up where their components say.
#[test]
fn rotated_churn() {
    let (mut world, e) = rotated();
    world.add_component(e[3], C);
    world.add_component(e[9], C);
    world.remove::<(C,)>(e[5]);
    world.remove::<(B,)>(e[11]);
    world.add_component(e[7], B(7));

    check(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that many rounds of pseudo-random B and C additions, removals and deletions over
/// rotated overlapping ranges, each followed by a two-phase regroup, keep every group aligned
/// and every entity where its components say. Ranges grow, shrink, move and rotate.
#[test]
fn rounds() {
    let (mut world, mut e) = rotated();
    let mut state = 0x2545_F491_u32;
    let mut next = |bound: usize| {
        // Xorshift, deterministic so a failure reproduces.
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as usize % bound
    };
    let rounds = if cfg!(miri) { 4 } else { 40 };

    for round in 0..rounds {
        for _ in 0..6 {
            let position = next(e.len());
            let entity = e[position];
            let value = entity.index() as u32;

            match next(6) {
                0 => world.add_component(entity, B(value)),
                1 => world.add_component(entity, C),
                2 => {
                    world.remove::<(B,)>(entity);
                }
                3 => {
                    world.remove::<(C,)>(entity);
                }
                4 => {
                    // The new entity reuses the deleted one's index with a new generation.
                    world.delete_entity(entity);
                    e[position] = world.add_entity((A(value), B(value)));
                }
                _ => {
                    world.remove::<(A,)>(entity);
                    world.add_component(entity, A(value));
                }
            }
        }

        let before = values(&world);
        two_phases(&world);
        assert_regrouped(&world);
        assert_memberships(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
        assert_eq!(values(&world), before, "round {round}");
    }
}
