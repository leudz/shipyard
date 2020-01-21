use shipyard::prelude::*;

#[test]
fn basic() {
    let world = World::new();

    world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        entities.add_entity(&mut u32s, 0);
        entities.add_entity(&mut u32s, 1);
        entities.add_entity(&mut u32s, 2);
    });

    let mut vec = Vec::new();
    world.run::<&u32, _, _>(|u32s| {
        u32s.iter().for_each(|&x| vec.push(x));
    });
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s).iter().for_each(|&mut x| vec.push(x));
    });

    assert_eq!(vec, vec![0, 1, 2, 0, 1, 2]);
}

#[test]
fn with_id() {
    let world = World::new();

    let (key0, key1, key2) =
        world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
            (
                entities.add_entity(&mut u32s, 0),
                entities.add_entity(&mut u32s, 1),
                entities.add_entity(&mut u32s, 2),
            )
        });

    let mut vec = Vec::new();
    world.run::<&u32, _, _>(|u32s| {
        u32s.iter().with_id().for_each(|(id, &x)| vec.push((id, x)));
    });
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s)
            .iter()
            .with_id()
            .for_each(|(id, &mut x)| vec.push((id, x)));
    });

    assert_eq!(
        vec,
        vec![
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

    world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        entities.add_entity(&mut u32s, 0);
        entities.add_entity(&mut u32s, 1);
        entities.add_entity(&mut u32s, 2);
    });

    let mut vec = Vec::new();
    world.run::<&u32, _, _>(|u32s| {
        u32s.iter().map(|x| *x + 10).for_each(|x| vec.push(x));
    });
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s).iter().map(|x| *x + 1).for_each(|x| vec.push(x));
    });

    assert_eq!(vec, vec![10, 11, 12, 1, 2, 3]);
}

#[test]
fn filter() {
    let world = World::new();

    world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        entities.add_entity(&mut u32s, 0);
        entities.add_entity(&mut u32s, 1);
        entities.add_entity(&mut u32s, 2);
    });

    let mut vec = Vec::new();
    world.run::<&u32, _, _>(|u32s| {
        u32s.iter()
            .filter(|&&x| x % 2 == 0)
            .for_each(|&x| vec.push(x));
    });
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s)
            .iter()
            .filter(|&&mut x| x % 2 != 0)
            .for_each(|&mut x| vec.push(x));
    });

    assert_eq!(vec, vec![0, 2, 1]);
}

#[test]
fn enumerate_map_filter_with_id() {
    let world = World::new();

    let (key0, _, key2) = world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        let result = (
            entities.add_entity(&mut u32s, 10),
            entities.add_entity(&mut u32s, 11),
            entities.add_entity(&mut u32s, 12),
        );
        result
    });

    let mut vec = Vec::new();
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s)
            .iter()
            .enumerate()
            .map(|(i, x)| (i * 3, x))
            .filter(|&(i, _)| i % 2 == 0)
            .with_id()
            .for_each(|(id, (i, &mut x))| vec.push((i, id, x)));
    });

    assert_eq!(vec, vec![(0, key0, 10), (6, key2, 12)]);
}

#[test]
fn enumerate_filter_map_with_id() {
    let world = World::new();

    let (key0, _, key2) = world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        let result = (
            entities.add_entity(&mut u32s, 10),
            entities.add_entity(&mut u32s, 11),
            entities.add_entity(&mut u32s, 12),
        );
        result
    });

    let mut vec = Vec::new();
    world.run::<&mut u32, _, _>(|mut u32s| {
        (&mut u32s)
            .iter()
            .enumerate()
            .filter(|&(i, _)| i % 2 == 0)
            .map(|(i, x)| (i * 3, x))
            .with_id()
            .for_each(|(id, (i, &mut x))| vec.push((i, id, x)));
    });

    assert_eq!(vec, vec![(0, key0, 10), (6, key2, 12)]);
}
