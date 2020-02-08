use core::any::type_name;
use shipyard::error;
use shipyard::internal::iterators;
use shipyard::prelude::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    usizes.delete(entity1);
    assert_eq!(
        (&mut usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!((&mut u32s).get(entity1), Ok(&mut 1));
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    Delete::<(usize,)>::delete((&mut usizes, &mut u32s), entity1);
    assert_eq!(
        (&mut usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!((&mut u32s).get(entity1), Ok(&mut 1));
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
    let iter = (&usizes, &u32s).iter();
    if let iterators::Iter2::Tight(mut iter) = iter {
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    } else {
        panic!("not packed");
    }
}

#[test]
fn loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    Delete::<(usize,)>::delete((&mut usizes, &mut u32s), entity1);
    assert_eq!(
        (&mut usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!((&mut u32s).get(entity1), Ok(&mut 1));
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&2, &3)));
    assert_eq!(iter.next(), None);
}

#[test]
fn tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) =
        world.borrow::<(EntitiesMut, &mut usize, &mut u64, &mut u32)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));
    entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
    Delete::<(u32,)>::delete((&mut u32s, &mut usizes, &mut u64s), entity1);
    let mut iter = (&usizes, &u64s).iter();
    assert_eq!(iter.next(), Some((&0, &1)));
    assert_eq!(iter.next(), Some((&3, &4)));
    assert_eq!(iter.next(), Some((&6, &7)));
    assert_eq!(iter.next(), None);
    let iter = (&usizes, &u64s, &u32s).iter();
    if let iterators::Iter3::Loose(mut iter) = iter {
        assert_eq!(iter.next(), Some((&6, &7, &8)));
        assert_eq!(iter.next(), Some((&3, &4, &5)));
        assert_eq!(iter.next(), None);
    }
    let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s, &mut u64s), entity2);
    assert_eq!(component, (Some(3),));
    let mut iter = (&usizes, &u64s).iter();
    assert_eq!(iter.next(), Some((&0, &1)));
    assert_eq!(iter.next(), Some((&6, &7)));
    assert_eq!(iter.next(), None);
    let mut iter = (&usizes, &u64s, &u32s).iter();
    assert_eq!(iter.next(), Some((&6, &7, &8)));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesMut, &mut usize)>();

    usizes.update_pack();

    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    usizes.delete(entity1);
    assert_eq!(
        usizes.get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.len(), 1);
    assert_eq!(usizes.inserted().len(), 1);
    assert_eq!(usizes.modified().len(), 0);
    assert_eq!(usizes.deleted().len(), 1);
    assert_eq!(usizes.take_deleted(), vec![(entity1, 0)]);
}

#[test]
fn strip() {
    let world = World::new();

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.strip(entity1);
    });

    world.run::<(&mut usize, &mut u32), _, _>(|(mut usizes, mut u32s)| {
        assert_eq!(
            (&mut usizes).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<usize>(),
            })
        );
        assert_eq!(
            (&mut u32s).get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<u32>(),
            })
        );
        assert_eq!(usizes.get(entity2), Ok(&2));
        assert_eq!(u32s.get(entity2), Ok(&3));
    });

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
    });
}
