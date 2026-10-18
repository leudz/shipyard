use shipyard::*;

#[allow(unused)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn key_equality() {
    let world = World::new();

    //create 3 entities
    let (e0, e1, e2) = world.run(
        |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<USIZE>)| {
            (
                entities.add_entity(&mut usizes, USIZE(0)),
                entities.add_entity(&mut usizes, USIZE(1)),
                entities.add_entity(&mut usizes, USIZE(2)),
            )
        },
    );

    //add a component to e1
    world.run(
        |(ref mut entities, ref mut u32s): (EntitiesViewMut, ViewMut<U32>)| {
            entities.add_component(e1, u32s, U32(42));
        },
    );

    //confirm that the entity keys have not changed for usizes storage
    world.run(|usizes: View<USIZE>| {
        //sanity check
        assert_eq!((&usizes).iter().with_id().count(), 3);

        let keys: Vec<EntityId> =
            (&usizes)
                .iter()
                .with_id()
                .map(|(entity, _)| entity)
                .fold(Vec::new(), |mut vec, x| {
                    vec.push(x);
                    vec
                });

        assert_eq!(keys, vec![e0, e1, e2]);
    });

    //confirm that the entity id for (USIZE) is the same as (USIZE, U32)
    //in other words that the entity itself did not somehow change from adding a component
    world.run(|(usizes, u32s): (View<USIZE>, View<U32>)| {
        //sanity check
        assert_eq!((&usizes, &u32s).iter().with_id().count(), 1);

        let (entity, (_, _)) = (&usizes, &u32s).iter().with_id().find(|_| true).unwrap();
        assert_eq!(entity, e1);
    });
}

#[test]
fn key_gen_after_slot_reuse() {
    let mut world = World::new();

    // Free the id without deleting its components, the storages keep the old generation.
    let e0 = world.add_entity((USIZE(0), U32(0)));
    world.run(|mut entities: EntitiesViewMut| {
        assert!(entities.delete_unchecked(e0));
    });

    // Reuse the same slot twice, `gen 1` then `gen 2`.
    let e1 = world.add_entity((USIZE(1), U32(1)));
    world.run(|mut entities: EntitiesViewMut| {
        assert!(entities.delete_unchecked(e1));
    });
    let e2 = world.add_entity((USIZE(2), U32(2)));

    assert_eq!(e2.index(), e0.index());
    assert_eq!(e2.generation(), 2);

    world.run(|(usizes, u32s): (View<USIZE>, View<U32>)| {
        // `dense` has to hold `e2` and not `gen 1 | gen 2 == gen 3`.
        let keys: Vec<EntityId> = (&usizes)
            .iter()
            .with_id()
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(keys, vec![e2]);

        // A corrupted key makes the lookup in the other storage fail and skips the entity.
        assert_eq!((&usizes, &u32s).iter().with_id().count(), 1);
    });
}
