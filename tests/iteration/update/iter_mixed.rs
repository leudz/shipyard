use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct I16(i16);
impl Component for I16 {
    type Tracking = track::Untracked;
}

#[test]
fn basic() {
    let mut world = World::new();
    world.track_all::<U32>();

    world.run(
        |(mut entities, mut u32s, mut i16s): (
            EntitiesViewMut,
            ViewMut<U32, track::All>,
            ViewMut<I16>,
        )| {
            entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
            entities.add_entity(&mut u32s, U32(1));
            entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
            entities.add_entity(&mut i16s, I16(13));

            assert_eq!(u32s.inserted().iter().count(), 3);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 3);

            u32s.clear_all_inserted();
        },
    );

    world.run(
        |(mut entities, mut u32s, mut i16s): (
            EntitiesViewMut,
            ViewMut<U32, track::All>,
            ViewMut<I16>,
        )| {
            assert_eq!(u32s.inserted().iter().count(), 0);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 0);

            entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&u32s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert_eq!(iter.next().unwrap(), &U32(0));
            assert_eq!(iter.next().unwrap(), &U32(1));
            assert_eq!(iter.next().unwrap(), &U32(2));
            assert_eq!(iter.next().unwrap(), &U32(4));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut u32s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            let mut next = iter.next().unwrap();
            next.0 += 1;
            next.0 -= 1;
            assert_eq!(*next, U32(0));
            assert_eq!(*iter.next().unwrap(), U32(1));
            assert_eq!(*iter.next().unwrap(), U32(2));
            assert_eq!(*iter.next().unwrap(), U32(4));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);

            u32s.clear_all_modified();
        },
    );

    world.run(
        |(mut u32s, mut i16s): (ViewMut<U32, track::All>, ViewMut<I16>)| {
            let mut iter = (&i16s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert_eq!(iter.next().unwrap(), &I16(10));
            assert_eq!(iter.next().unwrap(), &I16(12));
            assert_eq!(iter.next().unwrap(), &I16(13));
            assert_eq!(iter.next().unwrap(), &I16(14));
            assert!(iter.next().is_none());

            let mut iter = (&mut i16s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert!(*iter.next().unwrap() == I16(10));
            assert!(*iter.next().unwrap() == I16(12));
            assert!(*iter.next().unwrap() == I16(13));
            assert!(*iter.next().unwrap() == I16(14));
            assert!(iter.next().is_none());

            let mut iter = (&u32s, &i16s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            assert_eq!(iter.next().unwrap(), (&U32(0), &I16(10)));
            assert_eq!(iter.next().unwrap(), (&U32(2), &I16(12)));
            assert_eq!(iter.next().unwrap(), (&U32(4), &I16(14)));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut u32s, &mut i16s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.0).0 += 1;
            (next.0).0 -= 1;
            assert_eq!((*next.0, *next.1), (U32(0), I16(10)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U32(2), I16(12))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U32(4), I16(14))
            );
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);

            u32s.clear_all_modified();
        },
    );

    world.run(
        |(mut u32s, mut i16s): (ViewMut<U32, track::All>, ViewMut<I16>)| {
            let mut iter = (&i16s, &u32s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            assert_eq!(iter.next().unwrap(), (&I16(10), &U32(0)));
            assert_eq!(iter.next().unwrap(), (&I16(12), &U32(2)));
            assert_eq!(iter.next().unwrap(), (&I16(14), &U32(4)));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut i16s, &mut u32s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.1).0 += 1;
            (next.1).0 -= 1;
            assert_eq!((*next.0, *next.1), (I16(10), U32(0)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(12), U32(2))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(14), U32(4))
            );
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);
        },
    );
}

#[test]
fn not_inserted() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u32s.inserted()).iter();

    assert_eq!(iter.next(), Some(&U32(0)));
    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U32(0), &I16(10))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_modified() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    let e1 = entities.add_entity(&mut u32s, U32(1));
    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    u32s[e0].0 += 100;
    u32s[e1].0 += 100;

    let mut iter = (!u32s.modified()).iter();

    assert_eq!(iter.next(), Some(&U32(2)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.modified(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U32(2), &I16(12))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_inserted_or_modified() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    u32s[e0].0 += 100;

    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u32s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted_or_modified(), &i16s).iter();

    assert_eq!(iter.next(), None);
}
