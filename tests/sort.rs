use shipyard::error;
use shipyard::iterators;
use shipyard::*;

#[test]
fn simple_sort() {
    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<usize>)| {
                entities.add_entity(&mut usizes, 5);
                entities.add_entity(&mut usizes, 2);
                entities.add_entity(&mut usizes, 4);
                entities.add_entity(&mut usizes, 3);
                entities.add_entity(&mut usizes, 1);

                usizes.sort().try_unstable(Ord::cmp).unwrap();

                let mut prev = 0;
                (&mut usizes).iter().for_each(|&mut x| {
                    assert!(prev <= x);
                    prev = x;
                });
            },
        )
        .unwrap();
}

#[test]
fn tight_sort() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
    entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
    entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
    entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

    (&mut usizes, &mut u32s)
        .sort()
        .try_unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)))
        .unwrap();

    let mut prev = 0;
    (&mut usizes, &mut u32s)
        .iter()
        .for_each(|(&mut x, &mut y)| {
            assert!(prev <= x + y as usize);
            prev = x + y as usize;
        });
}

#[test]
fn loose_sort() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();

    entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
    entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
    entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
    entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

    (&mut usizes, &mut u32s)
        .sort()
        .try_unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)))
        .unwrap();

    let mut prev = 0;
    (&mut usizes, &mut u32s)
        .iter()
        .for_each(|(&mut x, &mut y)| {
            assert!(prev <= x + y as usize);
            prev = x + y as usize;
        });
}

#[test]
fn tight_loose_sort() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes, &mut u64s)).unwrap();

    entities.add_entity((&mut usizes, &mut u64s), (3, 4));
    entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
    entities.add_entity((&mut usizes,), (5,));
    entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));

    (&mut usizes, &mut u64s)
        .sort()
        .try_unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)))
        .unwrap();

    if let iterators::Iter3::Loose(mut iter) = (&usizes, &u64s, &u32s).iter() {
        assert_eq!(iter.next(), Some((&6, &7, &8)));
        assert_eq!(iter.next(), Some((&0, &1, &2)));
        assert_eq!(iter.next(), None);
    } else {
        panic!("not loose");
    }
    if let iterators::Iter2::Tight(mut iter) = (&usizes, &u64s).iter() {
        assert_eq!(iter.next(), Some((&0, &1)));
        assert_eq!(iter.next(), Some((&3, &4)));
        assert_eq!(iter.next(), Some((&6, &7)));
        assert_eq!(iter.next(), None);
    } else {
        panic!("not tight");
    }
    if let iterators::Iter2::NonPacked(mut iter) = (&usizes, &u32s).iter() {
        assert_eq!(iter.next(), Some((&6, &8)));
        assert_eq!(iter.next(), Some((&0, &2)));
        assert_eq!(iter.next(), None);
    }
}

#[test]
fn tight_sort_missing_storage() {
    let world = World::new();
    let (mut usizes, mut u64s) = world
        .try_borrow::<(ViewMut<usize>, ViewMut<u64>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    assert_eq!(
        usizes.sort().try_unstable(Ord::cmp).err(),
        Some(error::Sort::MissingPackStorage)
    );
}

#[test]
fn loose_sort_missing_storage() {
    let world = World::new();
    let (mut usizes, mut u64s) = world
        .try_borrow::<(ViewMut<usize>, ViewMut<u64>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_loose_pack().unwrap();
    assert_eq!(
        usizes.sort().try_unstable(Ord::cmp).err(),
        Some(error::Sort::MissingPackStorage)
    );
}

#[test]
fn tight_sort_too_many_storages() {
    let world = World::new();
    let (mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    assert_eq!(
        (&mut usizes, &mut u64s, &mut u32s)
            .sort()
            .try_unstable(|(&x1, &y1, &z1), (&x2, &y2, &z2)| {
                (x1 + y1 as usize + z1 as usize).cmp(&(x2 + y2 as usize + z2 as usize))
            })
            .err(),
        Some(error::Sort::TooManyStorages)
    );
}

#[test]
fn update_sort() {
    let world = World::new();
    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();

    usizes.try_update_pack().unwrap();
    assert_eq!(
        usizes.sort().try_unstable(Ord::cmp).err(),
        Some(error::Sort::MissingPackStorage)
    );
}
