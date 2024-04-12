use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::All;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct I16(i16);
impl Component for I16 {
    type Tracking = track::Untracked;
}

#[test]
fn basic() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.run(
        |(mut entities, mut u64s, mut i16s): (EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)| {
            entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
            entities.add_entity(&mut u64s, U64(1));
            entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
            entities.add_entity(&mut i16s, I16(13));

            assert_eq!(u64s.inserted().iter().count(), 3);
            assert_eq!(u64s.modified().iter().count(), 0);
            assert_eq!(u64s.inserted_or_modified().iter().count(), 3);

            u64s.clear_all_inserted();
        },
    );

    world.run(
        |(mut entities, mut u64s, mut i16s): (EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)| {
            assert_eq!(u64s.inserted().iter().count(), 0);
            assert_eq!(u64s.modified().iter().count(), 0);
            assert_eq!(u64s.inserted_or_modified().iter().count(), 0);

            entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

            assert_eq!(u64s.inserted().iter().count(), 1);
            assert_eq!(u64s.modified().iter().count(), 0);
            assert_eq!(u64s.inserted_or_modified().iter().count(), 1);

            if let iter::Iter::Tight(mut iter) = (&u64s).iter() {
                assert_eq!(iter.size_hint(), (4, Some(4)));
                assert_eq!(iter.next().unwrap(), &U64(0));
                assert_eq!(iter.next().unwrap(), &U64(1));
                assert_eq!(iter.next().unwrap(), &U64(2));
                assert_eq!(iter.next().unwrap(), &U64(4));
                assert!(iter.next().is_none());
            } else {
                panic!()
            }

            assert_eq!(u64s.inserted().iter().count(), 1);
            assert_eq!(u64s.modified().iter().count(), 0);
            assert_eq!(u64s.inserted_or_modified().iter().count(), 1);

            if let iter::Iter::Tight(mut iter) = (&mut u64s).iter() {
                assert_eq!(iter.size_hint(), (4, Some(4)));
                let mut next = iter.next().unwrap();
                next.0 += 1;
                next.0 -= 1;
                assert_eq!(*next, U64(0));
                assert_eq!(*iter.next().unwrap(), U64(1));
                assert_eq!(*iter.next().unwrap(), U64(2));
                assert_eq!(*iter.next().unwrap(), U64(4));
                assert!(iter.next().is_none());
            } else {
                panic!()
            }

            assert_eq!(u64s.inserted().iter().count(), 1);
            assert_eq!(u64s.modified().iter().count(), 1);
            assert_eq!(u64s.inserted_or_modified().iter().count(), 2);

            u64s.clear_all_modified();
        },
    );

    world.run(|(mut u64s, mut i16s): (ViewMut<U64>, ViewMut<I16>)| {
        if let iter::Iter::Tight(mut iter) = (&i16s).iter() {
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert_eq!(iter.next().unwrap(), &I16(10));
            assert_eq!(iter.next().unwrap(), &I16(12));
            assert_eq!(iter.next().unwrap(), &I16(13));
            assert_eq!(iter.next().unwrap(), &I16(14));
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        if let iter::Iter::Tight(mut iter) = (&mut i16s).iter() {
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert!(*iter.next().unwrap() == I16(10));
            assert!(*iter.next().unwrap() == I16(12));
            assert!(*iter.next().unwrap() == I16(13));
            assert!(*iter.next().unwrap() == I16(14));
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        if let iter::Iter::Mixed(mut iter) = (&u64s, &i16s).iter() {
            assert_eq!(iter.size_hint(), (0, Some(4)));
            assert_eq!(iter.next().unwrap(), (&U64(0), &I16(10)));
            assert_eq!(iter.next().unwrap(), (&U64(2), &I16(12)));
            assert_eq!(iter.next().unwrap(), (&U64(4), &I16(14)));
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        assert_eq!(u64s.inserted().iter().count(), 1);
        assert_eq!(u64s.modified().iter().count(), 0);
        assert_eq!(u64s.inserted_or_modified().iter().count(), 1);

        if let iter::Iter::Mixed(mut iter) = (&mut u64s, &mut i16s).iter() {
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.0).0 += 1;
            (next.0).0 -= 1;
            assert_eq!((*next.0, *next.1), (U64(0), I16(10)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U64(2), I16(12))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U64(4), I16(14))
            );
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        assert_eq!(u64s.inserted().iter().count(), 1);
        assert_eq!(u64s.modified().iter().count(), 1);
        assert_eq!(u64s.inserted_or_modified().iter().count(), 2);

        u64s.clear_all_modified();
    });

    world.run(|(mut u64s, mut i16s): (ViewMut<U64>, ViewMut<I16>)| {
        if let iter::Iter::Mixed(mut iter) = (&i16s, &u64s).iter() {
            assert_eq!(iter.size_hint(), (0, Some(4)));
            assert_eq!(iter.next().unwrap(), (&I16(10), &U64(0)));
            assert_eq!(iter.next().unwrap(), (&I16(12), &U64(2)));
            assert_eq!(iter.next().unwrap(), (&I16(14), &U64(4)));
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        assert_eq!(u64s.inserted().iter().count(), 1);
        assert_eq!(u64s.modified().iter().count(), 0);
        assert_eq!(u64s.inserted_or_modified().iter().count(), 1);

        if let iter::Iter::Mixed(mut iter) = (&mut i16s, &mut u64s).iter() {
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.1).0 += 1;
            (next.1).0 -= 1;
            assert_eq!((*next.0, *next.1), (I16(10), U64(0)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(12), U64(2))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(14), U64(4))
            );
            assert!(iter.next().is_none());
        } else {
            panic!()
        }

        assert_eq!(u64s.inserted().iter().count(), 1);
        assert_eq!(u64s.modified().iter().count(), 1);
        assert_eq!(u64s.inserted_or_modified().iter().count(), 2);
    });
}

#[test]
fn not_inserted() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    entities.add_entity(&mut u64s, U64(1));

    u64s.clear_all_inserted();

    let mut u64s = world.borrow::<ViewMut<U64>>().unwrap();

    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u64s.inserted()).iter();

    assert_eq!(iter.next(), Some(&U64(0)));
    assert_eq!(iter.next(), Some(&U64(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u64s.inserted(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U64(0), &I16(10))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_modified() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    let e1 = entities.add_entity(&mut u64s, U64(1));
    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    u64s.clear_all_inserted();

    let mut u64s = world.borrow::<ViewMut<U64>>().unwrap();

    u64s[e0].0 += 100;
    u64s[e1].0 += 100;

    let mut iter = (!u64s.modified()).iter();

    assert_eq!(iter.next(), Some(&U64(2)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u64s.modified(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U64(2), &I16(12))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_inserted_or_modified() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    entities.add_entity(&mut u64s, U64(1));

    u64s.clear_all_inserted();

    let mut u64s = world.borrow::<ViewMut<U64>>().unwrap();

    u64s[e0].0 += 100;

    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u64s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some(&U64(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u64s.inserted_or_modified(), &i16s).iter();

    assert_eq!(iter.next(), None);
}
