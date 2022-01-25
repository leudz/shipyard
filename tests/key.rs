use shipyard::*;

struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn key_equality() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
