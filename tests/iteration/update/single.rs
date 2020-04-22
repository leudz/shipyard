use shipyard::*;

#[test]
fn basic() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);

    assert_eq!(u32s.inserted().len(), 3);
    assert_eq!(u32s.modified().len(), 0);
    assert_eq!(u32s.inserted_or_modified().len(), 3);

    u32s.clear_inserted();

    assert_eq!(u32s.inserted().len(), 0);
    assert_eq!(u32s.modified().len(), 0);
    assert_eq!(u32s.inserted_or_modified().len(), 0);

    drop(u32s);
    let mut vec = Vec::new();
    world.run(|u32s: View<u32>| {
        let iter = u32s.iter();
        assert_eq!(iter.size_hint(), (3, Some(3)));
        iter.for_each(|&x| vec.push(x));

        assert_eq!(u32s.inserted().len(), 0);
        assert_eq!(u32s.modified().len(), 0);
        assert_eq!(u32s.inserted_or_modified().len(), 0);
    });
    world.run(|mut u32s: ViewMut<u32>| {
        (&mut u32s).iter().for_each(|&mut x| vec.push(x));
        u32s.modified().iter().for_each(|&x| vec.push(x));
        (&mut u32s).iter().for_each(|_| {});

        assert_eq!(u32s.inserted().len(), 0);
        assert_eq!(u32s.modified().len(), 3);
        assert_eq!(u32s.inserted_or_modified().len(), 3);
    });

    assert_eq!(vec, vec![0, 1, 2, 0, 1, 2, 0, 1, 2]);
}

#[test]
fn with_id() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    let key0 = entities.add_entity(&mut u32s, 0);
    let key1 = entities.add_entity(&mut u32s, 1);
    let key2 = entities.add_entity(&mut u32s, 2);
    u32s.clear_inserted();

    drop(u32s);
    let mut vec = Vec::new();
    world.run(|u32s: View<u32>| {
        u32s.iter().with_id().for_each(|(id, &x)| vec.push((id, x)));
    });
    world.run(|mut u32s: ViewMut<u32>| {
        (&mut u32s)
            .iter()
            .with_id()
            .for_each(|(id, &mut x)| vec.push((id, x)));
        u32s.modified()
            .iter()
            .with_id()
            .for_each(|(id, &x)| vec.push((id, x)));
    });

    assert_eq!(
        vec,
        vec![
            (key0, 0),
            (key1, 1),
            (key2, 2),
            (key0, 0),
            (key1, 1),
            (key2, 2),
            (key0, 0),
            (key1, 1),
            (key2, 2)
        ]
    );
}

#[test]
fn map() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);
    u32s.clear_inserted();

    drop(u32s);
    let mut vec = Vec::new();
    world.run(|u32s: View<u32>| {
        u32s.iter().map(|x| *x + 10).for_each(|x| vec.push(x));
    });
    world.run(|mut u32s: ViewMut<u32>| {
        (&mut u32s).iter().map(|x| *x + 1).for_each(|x| vec.push(x));
        u32s.modified().iter().for_each(|&x| vec.push(x));
    });

    assert_eq!(vec, vec![10, 11, 12, 1, 2, 3, 0, 1, 2]);
}

#[test]
fn filter() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);
    u32s.clear_inserted();

    assert_eq!((&u32s).iter().size_hint(), (3, Some(3)));
    assert_eq!(
        (&u32s).iter().filter(|&&x| x % 2 == 0).collect::<Vec<_>>(),
        vec![&0, &2]
    );
    assert_eq!(
        (&mut u32s)
            .iter()
            .filter(|&&mut x| x % 2 != 0)
            .collect::<Vec<_>>(),
        vec![&mut 1]
    );
    assert_eq!(u32s.modified().iter().collect::<Vec<_>>(), vec![&1]);
}

#[test]
fn enumerate_map_filter_with_id() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    let key0 = entities.add_entity(&mut u32s, 10);
    entities.add_entity(&mut u32s, 11);
    let key2 = entities.add_entity(&mut u32s, 12);
    u32s.clear_inserted();

    drop(u32s);
    let mut vec = Vec::new();
    let mut modified = Vec::new();
    world.run(|mut u32s: ViewMut<u32>| {
        (&mut u32s)
            .iter()
            .enumerate()
            .map(|(i, x)| (i * 3, x))
            .filter(|&(i, _)| i % 2 == 0)
            .with_id()
            .for_each(|(id, (i, &mut x))| vec.push((i, id, x)));
        u32s.modified().iter().for_each(|&x| modified.push(x));
    });

    assert_eq!(vec, vec![(0, key0, 10), (6, key2, 12)]);
    assert_eq!(modified, vec![10, 11, 12]);
}

#[test]
fn enumerate_filter_map_with_id() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();

    u32s.update_pack();
    let key0 = entities.add_entity(&mut u32s, 10);
    entities.add_entity(&mut u32s, 11);
    let key2 = entities.add_entity(&mut u32s, 12);
    u32s.clear_inserted();

    drop(u32s);
    let mut vec = Vec::new();
    let mut modified = Vec::new();
    world.run(|mut u32s: ViewMut<u32>| {
        (&mut u32s)
            .iter()
            .enumerate()
            .filter(|&(i, _)| i % 2 == 0)
            .map(|(i, x)| (i * 3, x))
            .with_id()
            .for_each(|(id, (i, &mut x))| vec.push((i, id, x)));
        u32s.modified().iter().for_each(|&x| modified.push(x));
    });

    assert_eq!(vec, vec![(0, key0, 10), (6, key2, 12)]);
    assert_eq!(modified, vec![10, 12]);
}
