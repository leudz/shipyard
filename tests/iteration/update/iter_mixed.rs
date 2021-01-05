use shipyard::*;

#[test]
fn basic() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    u32s.update_pack();

    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);

    assert_eq!(u32s.inserted().iter().count(), 3);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 3);
    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    u32s.clear_all_inserted();

    assert_eq!(u32s.inserted().iter().count(), 0);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 0);

    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

    if let iter::Iter::Tight(mut iter) = (&u32s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.next().unwrap(), &0);
        assert_eq!(iter.next().unwrap(), &1);
        assert_eq!(iter.next().unwrap(), &2);
        assert_eq!(iter.next().unwrap(), &4);
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

    if let iter::Iter::Tight(mut iter) = (&mut u32s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        let mut next = iter.next().unwrap();
        *next += 1;
        *next -= 1;
        assert_eq!(*next, 0);
        assert_eq!(*iter.next().unwrap(), 1);
        assert_eq!(*iter.next().unwrap(), 2);
        assert_eq!(*iter.next().unwrap(), 4);
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 1);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 2);

    u32s.clear_all_modified();

    if let iter::Iter::Tight(mut iter) = (&i16s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.next().unwrap(), &10);
        assert_eq!(iter.next().unwrap(), &12);
        assert_eq!(iter.next().unwrap(), &13);
        assert_eq!(iter.next().unwrap(), &14);
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    if let iter::Iter::Tight(mut iter) = (&mut i16s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert!(*iter.next().unwrap() == 10);
        assert!(*iter.next().unwrap() == 12);
        assert!(*iter.next().unwrap() == 13);
        assert!(*iter.next().unwrap() == 14);
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    if let iter::Iter::Mixed(mut iter) = (&u32s, &i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&0, &10));
        assert_eq!(iter.next().unwrap(), (&2, &12));
        assert_eq!(iter.next().unwrap(), (&4, &14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 1);
    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    if let iter::Iter::Mixed(mut iter) = (&mut u32s, &mut i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        let mut next = iter.next().unwrap();
        *next.0 += 1;
        *next.0 -= 1;
        assert_eq!((*next.0, *next.1), (0, 10));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (2, 12));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (4, 14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 1);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 2);
    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    u32s.clear_all_modified();

    if let iter::Iter::Mixed(mut iter) = (&i16s, &u32s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&10, &0));
        assert_eq!(iter.next().unwrap(), (&12, &2));
        assert_eq!(iter.next().unwrap(), (&14, &4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 0);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 1);
    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);

    if let iter::Iter::Mixed(mut iter) = (&mut i16s, &mut u32s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        let mut next = iter.next().unwrap();
        *next.1 += 1;
        *next.1 -= 1;
        assert_eq!((*next.0, *next.1), (10, 0));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (12, 2));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (14, 4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    assert_eq!(u32s.inserted().iter().count(), 1);
    assert_eq!(u32s.modified().iter().count(), 1);
    assert_eq!(u32s.inserted_or_modified().iter().count(), 2);
    assert_eq!(i16s.inserted().iter().count(), 0);
    assert_eq!(i16s.modified().iter().count(), 0);
    assert_eq!(i16s.inserted_or_modified().iter().count(), 0);
}

#[test]
fn not_inserted() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    u32s.update_pack();

    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);

    u32s.clear_all_inserted();

    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);

    let mut iter = (!u32s.inserted()).iter();

    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    let mut iter = (!i16s.inserted()).iter();

    assert_eq!(iter.next(), Some(&10));
    assert_eq!(iter.next(), Some(&12));
    assert_eq!(iter.next(), Some(&13));
    assert_eq!(iter.next(), None);

    let mut iter = (&u32s, !i16s.inserted()).iter();

    assert_eq!(iter.next(), Some((&0, &10)));
    assert_eq!(iter.next(), Some((&2, &12)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted(), &i16s).iter();

    assert_eq!(iter.next(), Some((&0, &10)));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_modified() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let e1 = entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);

    u32s.update_pack();

    u32s[e0] += 100;
    u32s[e1] += 100;

    let mut iter = (!u32s.modified()).iter();

    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);

    let mut iter = (!i16s.modified()).iter();

    assert_eq!(iter.next(), Some(&10));
    assert_eq!(iter.next(), Some(&12));
    assert_eq!(iter.next(), Some(&13));
    assert_eq!(iter.next(), None);

    let mut iter = (&u32s, !i16s.modified()).iter();

    assert_eq!(iter.next(), Some((&100, &10)));
    assert_eq!(iter.next(), Some((&2, &12)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.modified(), &i16s).iter();

    assert_eq!(iter.next(), Some((&2, &12)));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_inserted_or_modified() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    u32s.update_pack();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);

    u32s.clear_all_inserted();

    u32s[e0] += 100;

    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);

    let mut iter = (!u32s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    let mut iter = (!i16s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some(&10));
    assert_eq!(iter.next(), Some(&12));
    assert_eq!(iter.next(), Some(&13));
    assert_eq!(iter.next(), None);

    let mut iter = (&u32s, !i16s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some((&100, &10)));
    assert_eq!(iter.next(), Some((&2, &12)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted_or_modified(), &i16s).iter();

    assert_eq!(iter.next(), None);
}
