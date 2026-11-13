use super::*;

/// Returns a world grouping AB and AC.
fn crossing() -> World {
    World::builder_for_test()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build()
}

/// Checks the plan and final memberships while preserving A and B payloads and live entities.
fn check(world: &mut World, declared: &[&[StorageId]]) {
    let state = |world: &World| {
        let (entities, a, b) = world
            .borrow::<(crate::EntitiesView<'_>, View<'_, A>, View<'_, B>)>()
            .unwrap();
        (
            sorted(entities.iter().collect()),
            a.iter()
                .with_id()
                .map(|(id, a)| (id, a.0))
                .collect::<BTreeMap<_, _>>(),
            b.iter()
                .with_id()
                .map(|(id, b)| (id, b.0))
                .collect::<BTreeMap<_, _>>(),
        )
    };
    let before = state(world);

    assert_plan(world, declared);
    world.regroup();

    assert_regrouped(world);
    assert_memberships(world, declared);
    assert_eq!(state(world), before);
}

/// Asserts that AB and AC classifications follow ownership before and after analysis attaches ABC.
#[test]
fn compiled_after_attach() {
    let mut world = crossing();
    for i in 0..4 {
        world.add_entity(A(i));
        world.add_entity((A(i), B(i)));
        world.add_entity((A(i), C));
        world.add_entity((A(i), B(i), C));
        world.add_entity((B(i), C));
    }

    assert_classified(&world);
    assert_plan(&world, &[&ab(), &[id::<A>(), id::<C>()]]);
    assert_eq!(groups_of::<C>(&world).len(), 2);
    assert_classified(&world);
    check(&mut world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that owning both AB and CD doesn't merge those disconnected groups in learned paths.
#[test]
fn compiled_disconnected() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C, D));
    world.add_entity((A(1), C, D));
    world.add_entity((A(2), C));
    attach_ids(&world, &[id::<A>(), id::<B>()]);
    attach_ids(&world, &[id::<C>(), id::<D>()]);

    assert_classified(&world);
    check(&mut world, &[&ab(), &[id::<C>(), id::<D>()]]);
}

/// Asserts that a fresh regroup classifies changed component combinations with new trees.
#[test]
fn compiled_second_regroup() {
    let mut world = crossing();
    let entities: Vec<EntityId> = (0..8).map(|i| world.add_entity((A(i), B(i), C))).collect();
    world.regroup();

    for &entity in entities.iter().step_by(2) {
        world.remove::<(C,)>(entity);
    }
    for &entity in entities.iter().skip(1).step_by(3) {
        world.remove::<(B,)>(entity);
    }

    check(&mut world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that scattered B removals leave exactly the surviving AB memberships and payloads.
#[test]
fn compiled_removal() {
    let (mut world, entities) = pairs(64);
    for &entity in entities.iter().step_by(3) {
        world.remove::<(B,)>(entity);
    }

    check(&mut world, &[&ab()]);
}

/// Asserts that scattered A removals across AB, AC and ABC preserve both joins and payloads.
#[test]
fn compiled_cross_removal() {
    let mut world = crossing();
    let entities: Vec<EntityId> = (0..64)
        .map(|i| match i % 4 {
            0 => world.add_entity(A(i)),
            1 => world.add_entity((A(i), B(i))),
            2 => world.add_entity((A(i), C)),
            _ => world.add_entity((A(i), B(i), C)),
        })
        .collect();
    world.regroup();

    for &entity in entities.iter().skip(1).step_by(5) {
        world.remove::<(A,)>(entity);
    }

    check(&mut world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that entities gaining B or C join the unions their new component ownership requires.
#[test]
fn compiled_addition() {
    let mut world = crossing();
    let entities: Vec<EntityId> = (0..64)
        .map(|i| match i % 3 {
            0 => world.add_entity(A(i)),
            1 => world.add_entity((A(i), B(i))),
            _ => world.add_entity((A(i), C)),
        })
        .collect();
    world.regroup();

    for (i, &entity) in (100..).zip(&entities).step_by(2) {
        world.add_component(entity, B(i));
    }
    for &entity in entities.iter().skip(1).step_by(3) {
        world.add_component(entity, C);
    }

    check(&mut world, &[&ab(), &[id::<A>(), id::<C>()]]);
}

/// Asserts that a learned path probes a repeatedly absent member only once on the next entity.
#[test]
fn compiled_walk() {
    let mut world = World::new_for_test();
    world.add_entity((A(0), B(0), C, D));
    let first = world.add_entity((A(1), B(1), C));
    let second = world.add_entity((A(2), B(2), C));
    attach_ids(&world, &[id::<A>(), id::<B>(), id::<D>()]);
    attach_ids(&world, &[id::<A>(), id::<C>(), id::<D>()]);

    let mut all_storages = world.all_storages_mut().unwrap();
    let selected = Selected::all(&mut all_storages.storages.get_mut().components_and_custom);
    let frozen = Frozen::new(&selected);
    let index = frozen.memberships.position(&id::<A>());
    let mut discovery = Discovery::default();

    assert_eq!(
        discovery.classify(&frozen.memberships, index, first, |_| 0),
        None
    );
    let learned = discovery.checks;
    assert_eq!(
        discovery.classify(&frozen.memberships, index, second, |_| 0),
        None
    );
    let walked = discovery.checks - learned;

    assert_eq!(walked + 1, learned);
}

/// Asserts that learning the absent branch after an AB match reuses the walk's B lookup.
#[test]
fn compiled_known() {
    let mut world = World::new_for_test();
    let paired = world.add_entity((A(0), B(0)));
    let lone = world.add_entity(A(1));
    attach_ids(&world, &[id::<A>(), id::<B>()]);

    let mut all_storages = world.all_storages_mut().unwrap();
    let selected = Selected::all(&mut all_storages.storages.get_mut().components_and_custom);
    let frozen = Frozen::new(&selected);
    let index = frozen.memberships.position(&id::<A>());
    let mut discovery = Discovery::default();

    assert_eq!(
        discovery.classify(&frozen.memberships, index, paired, |_| 0),
        Some(0)
    );
    assert_eq!(discovery.checks, 1);
    assert_eq!(
        discovery.classify(&frozen.memberships, index, lone, |_| 0),
        None
    );
    assert_eq!(discovery.checks, 2);
}

/// Asserts that forgetting learned AB results lets a rebuilt membership discover a new ABC union.
#[test]
fn compiled_forget() {
    let mut world = World::new_for_test();
    let entity = world.add_entity((A(0), B(0), C));
    attach_ids(&world, &ab());
    let mut discovery = Discovery::default();

    {
        let mut all_storages = world.all_storages_mut().unwrap();
        let selected = Selected::all(&mut all_storages.storages.get_mut().components_and_custom);
        let frozen = Frozen::new(&selected);
        let index = frozen.memberships.position(&id::<A>());
        let found = discovery.classify(&frozen.memberships, index, entity, |members| {
            assert_eq!(members, sorted_ids(ab().to_vec()));
            0
        });
        assert_eq!(found, Some(0));
    }

    attach_ids(&world, &[id::<A>(), id::<C>()]);
    discovery.forget();

    let mut all_storages = world.all_storages_mut().unwrap();
    let selected = Selected::all(&mut all_storages.storages.get_mut().components_and_custom);
    let frozen = Frozen::new(&selected);
    let index = frozen.memberships.position(&id::<A>());
    let found = discovery.classify(&frozen.memberships, index, entity, |members| {
        assert_eq!(members, sorted_ids(vec![id::<A>(), id::<B>(), id::<C>()]));
        1
    });

    assert_eq!(found, Some(1));
}
