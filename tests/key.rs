use shipyard::*;

#[test]
fn key_equality() {
    let world = World::default();

    //create 3 entities
    let (e0, e1, e2) = world
        .run(
            |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<usize>)| {
                (
                    entities.add_entity(&mut usizes, 0),
                    entities.add_entity(&mut usizes, 1),
                    entities.add_entity(&mut usizes, 2),
                )
            },
        )
        .unwrap();

    //add a component to e1
    world
        .run(
            |(ref mut entities, ref mut u32s): (EntitiesViewMut, ViewMut<u32>)| {
                entities.add_component(e1, u32s, 42);
            },
        )
        .unwrap();

    //confirm that the entity keys have not changed for usizes storage
    world
        .run(|usizes: View<usize>| {
            //sanity check
            assert_eq!((&usizes).iter().with_id().count(), 3);

            let keys: Vec<EntityId> = (&usizes).iter().with_id().map(|(entity, _)| entity).fold(
                Vec::new(),
                |mut vec, x| {
                    vec.push(x);
                    vec
                },
            );

            assert_eq!(keys, vec![e0, e1, e2]);
        })
        .unwrap();

    //confirm that the entity id for (usize) is the same as (usize, u32)
    //in other words that the entity itself did not somehow change from adding a component
    world
        .run(|(usizes, u32s): (View<usize>, View<u32>)| {
            //sanity check
            assert_eq!((&usizes, &u32s).iter().with_id().count(), 1);

            let (entity, (_, _)) = (&usizes, &u32s).iter().with_id().find(|_| true).unwrap();
            assert_eq!(entity, e1);
        })
        .unwrap();
}
