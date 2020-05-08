use shipyard::*;

#[test]
fn simple() {
    let world = World::new();

    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            let (entity0, entity1, entity2, entity3) = all_storages
                .try_run(
                    |mut entities: EntitiesViewMut,
                     mut u32s: ViewMut<u32>,
                     mut usizes: ViewMut<usize>| {
                        (
                            entities.add_entity(&mut u32s, 0),
                            entities.add_entity((), ()),
                            entities.add_entity(&mut usizes, 1),
                            entities.add_entity((&mut u32s, &mut usizes), (2, 3)),
                        )
                    },
                )
                .unwrap();

            all_storages.delete_any::<(u32,)>();

            all_storages
                .try_run(|entities: EntitiesView| {
                    assert!(!entities.is_alive(entity0));
                    assert!(entities.is_alive(entity1));
                    assert!(entities.is_alive(entity2));
                    assert!(!entities.is_alive(entity3));
                })
                .unwrap();
        })
        .unwrap();
}
