use shipyard::prelude::*;

#[test]
fn basic() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s.iter();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(iter.next().unwrap(), &0);
    assert_eq!(iter.next().unwrap(), &2);
    assert_eq!(iter.next().unwrap(), &4);
    assert_eq!(iter.next().unwrap(), &1);
    assert!(iter.next().is_none());
    let mut iter = u32s.iter();
    assert_eq!(iter.next().unwrap(), &mut 0);
    assert_eq!(iter.next().unwrap(), &mut 2);
    assert_eq!(iter.next().unwrap(), &mut 4);
    assert_eq!(iter.next().unwrap(), &mut 1);
    assert!(iter.next().is_none());

    let mut iter = i16s.iter();
    assert_eq!(iter.next().unwrap(), &10);
    assert_eq!(iter.next().unwrap(), &12);
    assert_eq!(iter.next().unwrap(), &14);
    assert_eq!(iter.next().unwrap(), &13);
    assert!(iter.next().is_none());
    let mut iter = i16s.iter();
    assert_eq!(iter.next().unwrap(), &mut 10);
    assert_eq!(iter.next().unwrap(), &mut 12);
    assert_eq!(iter.next().unwrap(), &mut 14);
    assert_eq!(iter.next().unwrap(), &mut 13);
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter();
    assert_eq!(iter.size_hint(), (3, Some(3)));
    assert_eq!(iter.next().unwrap(), (&0, &10));
    assert_eq!(iter.next().unwrap(), (&2, &12));
    assert_eq!(iter.next().unwrap(), (&4, &14));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter();
    assert_eq!(iter.next().unwrap(), (&mut 0, &mut 10));
    assert_eq!(iter.next().unwrap(), (&mut 2, &mut 12));
    assert_eq!(iter.next().unwrap(), (&mut 4, &mut 14));
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter();
    assert_eq!(iter.next().unwrap(), (&10, &0));
    assert_eq!(iter.next().unwrap(), (&12, &2));
    assert_eq!(iter.next().unwrap(), (&14, &4));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter();
    assert_eq!(iter.next().unwrap(), (&mut 10, &mut 0));
    assert_eq!(iter.next().unwrap(), (&mut 12, &mut 2));
    assert_eq!(iter.next().unwrap(), (&mut 14, &mut 4));
    assert!(iter.next().is_none());
}

#[test]
fn with_id() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    let key0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let key1 = entities.add_entity(&mut u32s, 1);
    let key2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let key3 = entities.add_entity(&mut i16s, 13);
    let key4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s.iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, &0));
    assert_eq!(iter.next().unwrap(), (key2, &2));
    assert_eq!(iter.next().unwrap(), (key4, &4));
    assert_eq!(iter.next().unwrap(), (key1, &1));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, &mut 0));
    assert_eq!(iter.next().unwrap(), (key2, &mut 2));
    assert_eq!(iter.next().unwrap(), (key4, &mut 4));
    assert_eq!(iter.next().unwrap(), (key1, &mut 1));
    assert!(iter.next().is_none());

    let mut iter = i16s.iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, &10));
    assert_eq!(iter.next().unwrap(), (key2, &12));
    assert_eq!(iter.next().unwrap(), (key4, &14));
    assert_eq!(iter.next().unwrap(), (key3, &13));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, &mut 10));
    assert_eq!(iter.next().unwrap(), (key2, &mut 12));
    assert_eq!(iter.next().unwrap(), (key4, &mut 14));
    assert_eq!(iter.next().unwrap(), (key3, &mut 13));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, (&0, &10)));
    assert_eq!(iter.next().unwrap(), (key2, (&2, &12)));
    assert_eq!(iter.next().unwrap(), (key4, (&4, &14)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, (&mut 0, &mut 10)));
    assert_eq!(iter.next().unwrap(), (key2, (&mut 2, &mut 12)));
    assert_eq!(iter.next().unwrap(), (key4, (&mut 4, &mut 14)));
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, (&10, &0)));
    assert_eq!(iter.next().unwrap(), (key2, (&12, &2)));
    assert_eq!(iter.next().unwrap(), (key4, (&14, &4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (key0, (&mut 10, &mut 0)));
    assert_eq!(iter.next().unwrap(), (key2, (&mut 12, &mut 2)));
    assert_eq!(iter.next().unwrap(), (key4, (&mut 14, &mut 4)));
    assert!(iter.next().is_none());
}

#[test]
fn map() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s.iter().map(Clone::clone);
    assert_eq!(iter.next().unwrap(), 0);
    assert_eq!(iter.next().unwrap(), 2);
    assert_eq!(iter.next().unwrap(), 4);
    assert_eq!(iter.next().unwrap(), 1);
    assert!(iter.next().is_none());
    let mut iter = u32s.iter().map(|x| *x);
    assert_eq!(iter.next().unwrap(), 0);
    assert_eq!(iter.next().unwrap(), 2);
    assert_eq!(iter.next().unwrap(), 4);
    assert_eq!(iter.next().unwrap(), 1);
    assert!(iter.next().is_none());

    let mut iter = i16s.iter().map(Clone::clone);
    assert_eq!(iter.next().unwrap(), 10);
    assert_eq!(iter.next().unwrap(), 12);
    assert_eq!(iter.next().unwrap(), 14);
    assert_eq!(iter.next().unwrap(), 13);
    assert!(iter.next().is_none());
    let mut iter = i16s.iter().map(|x| *x);
    assert_eq!(iter.next().unwrap(), 10);
    assert_eq!(iter.next().unwrap(), 12);
    assert_eq!(iter.next().unwrap(), 14);
    assert_eq!(iter.next().unwrap(), 13);
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter().map(|(x, y)| (*x, *y));
    assert_eq!(iter.next().unwrap(), (0, 10));
    assert_eq!(iter.next().unwrap(), (2, 12));
    assert_eq!(iter.next().unwrap(), (4, 14));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter().map(|(x, y)| (*x, *y));
    assert_eq!(iter.next().unwrap(), (0, 10));
    assert_eq!(iter.next().unwrap(), (2, 12));
    assert_eq!(iter.next().unwrap(), (4, 14));
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter().map(|(x, y)| (*x, *y));
    assert_eq!(iter.next().unwrap(), (10, 0));
    assert_eq!(iter.next().unwrap(), (12, 2));
    assert_eq!(iter.next().unwrap(), (14, 4));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter().map(|(x, y)| (*x, *y));
    assert_eq!(iter.next().unwrap(), (10, 0));
    assert_eq!(iter.next().unwrap(), (12, 2));
    assert_eq!(iter.next().unwrap(), (14, 4));
    assert!(iter.next().is_none());
}

#[test]
fn filter() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s.iter().filter(|x| **x % 2 == 0);
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(iter.next().unwrap(), &0);
    assert_eq!(iter.next().unwrap(), &2);
    assert_eq!(iter.next().unwrap(), &4);
    assert!(iter.next().is_none());
    let mut iter = u32s.iter().filter(|x| **x % 2 != 0);
    assert_eq!(iter.next().unwrap(), &mut 1);
    assert!(iter.next().is_none());

    let mut iter = i16s.iter().filter(|x| **x % 2 == 0);
    assert_eq!(iter.next().unwrap(), &10);
    assert_eq!(iter.next().unwrap(), &12);
    assert_eq!(iter.next().unwrap(), &14);
    assert!(iter.next().is_none());
    let mut iter = i16s.iter().filter(|x| **x % 2 != 0);
    assert_eq!(iter.next().unwrap(), &mut 13);
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter().filter(|(_, y)| **y % 5 == 0);
    assert_eq!(iter.next().unwrap(), (&0, &10));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter().filter(|(_, y)| **y % 5 != 0);
    assert_eq!(iter.next().unwrap(), (&mut 2, &mut 12));
    assert_eq!(iter.next().unwrap(), (&mut 4, &mut 14));
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter().filter(|(x, _)| **x % 5 == 0);
    assert_eq!(iter.next().unwrap(), (&10, &0));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter().filter(|(x, _)| **x % 5 != 0);
    assert_eq!(iter.next().unwrap(), (&mut 12, &mut 2));
    assert_eq!(iter.next().unwrap(), (&mut 14, &mut 4));
    assert!(iter.next().is_none());
}

#[test]
fn enumerate_map_filter_with_id() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    let key0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let key1 = entities.add_entity(&mut u32s, 1);
    let key2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let key3 = entities.add_entity(&mut i16s, 13);
    let key4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|(_, x)| **x % 2 == 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, &0)));
    assert_eq!(iter.next().unwrap(), (key2, (3, &2)));
    assert_eq!(iter.next().unwrap(), (key4, (6, &4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s)
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|(_, x)| **x % 2 != 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key1, (9, &mut 1)));
    assert!(iter.next().is_none());

    let mut iter = i16s
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|(_, x)| **x % 2 == 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, &10)));
    assert_eq!(iter.next().unwrap(), (key2, (3, &12)));
    assert_eq!(iter.next().unwrap(), (key4, (6, &14)));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s)
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|(_, x)| **x % 2 != 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key3, (9, &mut 13)));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s)
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|&(i, _)| i % 2 == 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, (&0, &10))));
    assert_eq!(iter.next().unwrap(), (key4, (6, (&4, &14))));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s)
        .iter()
        .enumerate()
        .map(|(i, x)| (i * 3, x))
        .filter(|&(i, _)| i % 2 != 0)
        .with_id();

    assert_eq!(iter.next().unwrap(), (key2, (3, (&mut 12, &mut 2))));
    assert!(iter.next().is_none());
}

#[test]
fn enumerate_filter_map_with_id() {
    let world = World::new();
    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();

    (&mut u32s, &mut i16s).tight_pack();
    let key0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let key1 = entities.add_entity(&mut u32s, 1);
    let key2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let key3 = entities.add_entity(&mut i16s, 13);
    let key4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = u32s
        .iter()
        .enumerate()
        .filter(|(_, x)| **x % 2 == 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, &0)));
    assert_eq!(iter.next().unwrap(), (key2, (3, &2)));
    assert_eq!(iter.next().unwrap(), (key4, (6, &4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s)
        .iter()
        .enumerate()
        .filter(|(_, x)| **x % 2 != 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key1, (9, &mut 1)));
    assert!(iter.next().is_none());

    let mut iter = i16s
        .iter()
        .enumerate()
        .filter(|(_, x)| **x % 2 == 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, &10)));
    assert_eq!(iter.next().unwrap(), (key2, (3, &12)));
    assert_eq!(iter.next().unwrap(), (key4, (6, &14)));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s)
        .iter()
        .enumerate()
        .filter(|(_, x)| **x % 2 != 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key3, (9, &mut 13)));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s)
        .iter()
        .enumerate()
        .filter(|&(i, _)| i % 2 == 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key0, (0, (&0, &10))));
    assert_eq!(iter.next().unwrap(), (key4, (6, (&4, &14))));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s)
        .iter()
        .enumerate()
        .filter(|&(i, _)| i % 2 != 0)
        .map(|(i, x)| (i * 3, x))
        .with_id();

    assert_eq!(iter.next().unwrap(), (key2, (3, (&mut 12, &mut 2))));
    assert!(iter.next().is_none());
}

#[test]
fn off_by_one() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world.borrow::<(EntitiesMut, &mut u32, &mut i16)>();
    (&mut u32s, &mut i16s).tight_pack();

    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let u32_window = u32s.as_window(1..);
    let iter = (&u32_window, &i16s).iter();
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.collect::<Vec<_>>(), vec![(&2, &12), (&4, &14)]);

    let u32_window = u32_window.as_window(1..);
    let iter = (&u32_window, &i16s).iter();
    assert_eq!(iter.size_hint(), (0, Some(2)));
    assert_eq!(iter.collect::<Vec<_>>(), vec![(&4, &14)]);

    let i16_window = i16s.as_window(1..);
    let iter = (&u32s, &i16_window).iter();
    assert_eq!(iter.size_hint(), (0, Some(3)));
    assert_eq!(iter.collect::<Vec<_>>(), vec![(&2, &12), (&4, &14)]);

    let i16_window = i16_window.as_window(1..);
    let iter = (&u32s, &i16_window).iter();
    assert_eq!(iter.size_hint(), (0, Some(2)));
    assert_eq!(iter.collect::<Vec<_>>(), vec![(&4, &14)]);
}
