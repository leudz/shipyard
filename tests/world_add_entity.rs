use shipyard::*;

#[test]
fn no_pack() {
    let mut world = World::default();

    world.add_entity(());
    let entity1 = world.add_entity((0usize, 1u32));

    let (usizes, u32s) = world.try_borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!((&usizes, &u32s).get(entity1), Ok((&0, &1)));
}

#[test]
fn update() {
    let mut world = World::new();

    drop(world.try_borrow::<ViewMut<usize>>().unwrap().update_pack());

    let entity = world.add_entity((0usize,));

    let usizes = world.try_borrow::<View<usize>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], 0);
}

#[test]
fn cleared_update() {
    let mut world = World::new();

    drop(world.try_borrow::<ViewMut<usize>>().unwrap().update_pack());

    let entity1 = world.add_entity((1usize,));

    world
        .try_run(|mut usizes: ViewMut<usize>| {
            usizes.try_clear_inserted_and_modified().unwrap();
            assert_eq!(usizes.inserted().iter().count(), 0);
        })
        .unwrap();

    let entity2 = world.add_entity((2usize,));

    world
        .try_run(|usizes: View<usize>| {
            assert_eq!(usizes.inserted().iter().count(), 1);
            assert_eq!(*usizes.get(entity1).unwrap(), 1);
            assert_eq!(*usizes.get(entity2).unwrap(), 2);
        })
        .unwrap();
}

#[test]
fn modified_update() {
    let mut world = World::new();

    drop(world.try_borrow::<ViewMut<usize>>().unwrap().update_pack());

    let entity1 = world.add_entity((1usize,));

    world
        .try_run(|mut usizes: ViewMut<usize>| {
            usizes.try_clear_inserted_and_modified().unwrap();
            usizes[entity1] = 3;
        })
        .unwrap();

    let entity2 = world.add_entity((2usize,));

    world
        .try_run(|usizes: View<usize>| {
            assert_eq!(usizes.inserted().iter().count(), 1);
            assert_eq!(*usizes.get(entity1).unwrap(), 3);
            assert_eq!(*usizes.get(entity2).unwrap(), 2);
        })
        .unwrap();
}
