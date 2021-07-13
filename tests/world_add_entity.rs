use shipyard::*;

#[test]
fn no_pack() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity(());
    let entity1 = world.add_entity((0usize, 1u32));

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!((&usizes, &u32s).get(entity1), Ok((&0, &1)));
}

#[test]
fn update() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.borrow::<ViewMut<usize>>().unwrap().track_all();

    let entity = world.add_entity((0usize,));

    let usizes = world.borrow::<View<usize>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], 0);
}

#[test]
fn cleared_update() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.borrow::<ViewMut<usize>>().unwrap().track_all();

    let entity1 = world.add_entity((1usize,));

    world
        .run(|mut usizes: ViewMut<usize>| {
            usizes.clear_all_inserted_and_modified();
            assert_eq!(usizes.inserted().iter().count(), 0);
        })
        .unwrap();

    let entity2 = world.add_entity((2usize,));

    world
        .run(|usizes: View<usize>| {
            assert_eq!(usizes.inserted().iter().count(), 1);
            assert_eq!(*usizes.get(entity1).unwrap(), 1);
            assert_eq!(*usizes.get(entity2).unwrap(), 2);
        })
        .unwrap();
}

#[test]
fn modified_update() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.borrow::<ViewMut<usize>>().unwrap().track_all();

    let entity1 = world.add_entity((1usize,));

    world
        .run(|mut usizes: ViewMut<usize>| {
            usizes.clear_all_inserted_and_modified();
            usizes[entity1] = 3;
        })
        .unwrap();

    let entity2 = world.add_entity((2usize,));

    world
        .run(|usizes: View<usize>| {
            assert_eq!(usizes.inserted().iter().count(), 1);
            assert_eq!(*usizes.get(entity1).unwrap(), 3);
            assert_eq!(*usizes.get(entity2).unwrap(), 2);
        })
        .unwrap();
}

#[test]
fn bulk_single() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (i as u32,)))
        .collect::<Vec<_>>();

    let u32s = world.borrow::<View<u32>>().unwrap();
    let mut iter = u32s.iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), Some(&3));
    assert_eq!(iter.next(), Some(&4));
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
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entities = world
        .bulk_add_entity((0..5).map(|i| (i as u32, i as usize)))
        .collect::<Vec<_>>();

    let (u32s, usizes) = world.borrow::<(View<u32>, View<usize>)>().unwrap();
    let mut iter = (&u32s, &usizes).iter();
    assert_eq!(iter.next(), Some((&0, &0)));
    assert_eq!(iter.next(), Some((&1, &1)));
    assert_eq!(iter.next(), Some((&2, &2)));
    assert_eq!(iter.next(), Some((&3, &3)));
    assert_eq!(iter.next(), Some((&4, &4)));
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

    world.bulk_add_entity((0..5).map(|i| (i as u32, i as usize)));

    let (u32s, usizes) = world.borrow::<(View<u32>, View<usize>)>().unwrap();
    assert_eq!(u32s.len(), 10);
    assert_eq!(usizes.len(), 10);
}
