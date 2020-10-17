use shipyard::*;

#[test]
fn basic() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

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

    if let iter::Iter::Tight(mut iter) = (&mut u32s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(*iter.next().unwrap(), 0);
        assert_eq!(*iter.next().unwrap(), 1);
        assert_eq!(*iter.next().unwrap(), 2);
        assert_eq!(*iter.next().unwrap(), 4);
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

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

    if let iter::Iter::Mixed(mut iter) = (&u32s, &i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&0, &10));
        assert_eq!(iter.next().unwrap(), (&2, &12));
        assert_eq!(iter.next().unwrap(), (&4, &14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&mut u32s, &mut i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (0, 10));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (2, 12));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (4, 14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&i16s, &u32s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&10, &0));
        assert_eq!(iter.next().unwrap(), (&12, &2));
        assert_eq!(iter.next().unwrap(), (&14, &4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&mut i16s, &mut u32s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (10, 0));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (12, 2));
        assert_eq!(iter.next().map(|(x, y)| (*x, *y)).unwrap(), (14, 4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }
}

#[test]
fn with_id() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    let id0 = entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    let id1 = entities.add_entity(&mut u32s, 1);
    let id2 = entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    let id3 = entities.add_entity(&mut i16s, 13);
    let id4 = entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    let mut iter = (&u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, &0));
    assert_eq!(iter.next().unwrap(), (id1, &1));
    assert_eq!(iter.next().unwrap(), (id2, &2));
    assert_eq!(iter.next().unwrap(), (id4, &4));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s).iter().with_id();
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id0, 0));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id1, 1));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id2, 2));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id4, 4));
    assert!(iter.next().is_none());

    let mut iter = i16s.iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, &10));
    assert_eq!(iter.next().unwrap(), (id2, &12));
    assert_eq!(iter.next().unwrap(), (id3, &13));
    assert_eq!(iter.next().unwrap(), (id4, &14));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s).iter().with_id();
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id0, 10));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id2, 12));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id3, 13));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id4, 14));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&0, &10)));
    assert_eq!(iter.next().unwrap(), (id2, (&2, &12)));
    assert_eq!(iter.next().unwrap(), (id4, (&4, &14)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (0, 10))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (2, 12))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (4, 14))
    );
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&10, &0)));
    assert_eq!(iter.next().unwrap(), (id2, (&12, &2)));
    assert_eq!(iter.next().unwrap(), (id4, (&14, &4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (10, 0))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (12, 2))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (14, 4))
    );
    assert!(iter.next().is_none());
}

#[test]
fn empty() {
    let world = World::new();

    let (usizes, u32s) = world
        .try_borrow::<(ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    assert!(u32s.iter().next().is_none());
    assert!(u32s.iter().with_id().next().is_none());
    assert!(usizes.iter().next().is_none());
    assert!(usizes.iter().with_id().next().is_none());
    assert!((&usizes, &u32s).iter().next().is_none());
    assert!((&usizes, &u32s).iter().with_id().next().is_none());
}

#[test]
fn not() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<i16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (0, 10));
    entities.add_entity(&mut u32s, 1);
    entities.add_entity((&mut u32s, &mut i16s), (2, 12));
    entities.add_entity(&mut i16s, 13);
    entities.add_entity((&mut u32s, &mut i16s), (4, 14));

    if let iter::Iter::Mixed(mut iter) = (&u32s, !&i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&1, ()));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }
}
