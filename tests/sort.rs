use shipyard::internal::iterators;
use shipyard::prelude::*;

#[test]
fn simple_sort() {
    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        usizes.sort().unstable(Ord::cmp);

        let mut prev = 0;
        (&mut usizes).iter().for_each(|&mut x| {
            assert!(prev <= x);
            prev = x;
        });
    });
}

#[test]
fn tight_sort() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
            entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
            entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
            entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

            (&mut usizes, &mut u32s)
                .sort()
                .unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)));

            let mut prev = 0;
            (&mut usizes, &mut u32s)
                .iter()
                .for_each(|(&mut x, &mut y)| {
                    assert!(prev <= x + y as usize);
                    prev = x + y as usize;
                });
        },
    );
}

#[test]
fn loose_sort() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
            entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
            entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
            entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

            (&mut usizes, &mut u32s)
                .sort()
                .unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)));

            let mut prev = 0;
            (&mut usizes, &mut u32s)
                .iter()
                .for_each(|(&mut x, &mut y)| {
                    assert!(prev <= x + y as usize);
                    prev = x + y as usize;
                });
        },
    );
}

#[test]
fn tight_loose_sort() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u64s), (3, 4));
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
            entities.add_entity((&mut usizes,), (5,));
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));

            (&mut usizes, &mut u64s)
                .sort()
                .unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)));
        },
    );

    world.run::<(&usize, &u64, &u32), _, _>(|(usizes, u64s, u32s)| {
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
    });
}

#[test]
#[should_panic(
    expected = "The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."
)]
fn tight_sort_missing_storage() {
    let world = World::new::<(usize, u64)>();
    world.tight_pack::<(usize, u64)>();

    world.run::<(&mut usize,), _, _>(|(mut usizes,)| {
        &mut usizes.sort().unstable(Ord::cmp);
    });
}

#[test]
#[should_panic(
    expected = "The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."
)]
fn loose_sort_missing_storage() {
    let world = World::new::<(usize, u64)>();
    world.loose_pack::<(usize,), (u64,)>();

    world.run::<(&mut usize,), _, _>(|(mut usizes,)| {
        &mut usizes.sort().unstable(Ord::cmp);
    });
}

#[test]
#[should_panic(
    expected = "You provided too many storages non packed together. Only single storage and storages packed together can be sorted."
)]
fn tight_sort_too_many_storages() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();

    world.run::<(&mut usize, &mut u64, &mut u32), _, _>(|(mut usizes, mut u64s, mut u32s)| {
        (&mut usizes, &mut u64s, &mut u32s)
            .sort()
            .unstable(|(&x1, &y1, &z1), (&x2, &y2, &z2)| {
                (x1 + y1 as usize + z1 as usize).cmp(&(x2 + y2 as usize + z2 as usize))
            });
    });
}

#[test]
#[should_panic(
    expected = "Result::unwrap()` on an `Err` value: The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."
)]
fn update_sort() {
    let world = World::new::<(usize,)>();

    world.update_pack::<usize>();

    world.run::<&mut usize, _, _>(|mut usizes| {
        usizes.sort().unstable(Ord::cmp);
    });
}
