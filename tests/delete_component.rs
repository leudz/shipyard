use core::any::type_name;
use shipyard::error;
use shipyard::iterators;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    usizes.try_delete(entity1).unwrap();
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
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    Delete::<(usize,)>::try_delete((&mut usizes, &mut u32s), entity1).unwrap();
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
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    Delete::<(usize,)>::try_delete((&mut usizes, &mut u32s), entity1).unwrap();
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
    let (mut entities, mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes, &mut u64s)).unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));
    entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
    Delete::<(u32,)>::try_delete((&mut u32s, &mut usizes, &mut u64s), entity1).unwrap();
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
    let component =
        Remove::<(usize,)>::try_remove((&mut usizes, &mut u32s, &mut u64s), entity2).unwrap();
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
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();

    usizes.try_update_pack().unwrap();

    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    usizes.try_delete(entity1).unwrap();
    assert_eq!(
        usizes.get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.len(), 1);
    assert_eq!(usizes.try_inserted().unwrap().len(), 1);
    assert_eq!(usizes.try_modified().unwrap().len(), 0);
    assert_eq!(usizes.try_deleted().unwrap().len(), 1);
    assert_eq!(usizes.try_take_deleted().unwrap(), vec![(entity1, 0)]);
}

#[test]
fn strip() {
    let world = World::new();

    let (entity1, entity2) = world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                (
                    entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                    entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
                )
            },
        )
        .unwrap();

    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            all_storages.strip(entity1);
        })
        .unwrap();

    world
        .try_run(|(mut usizes, mut u32s): (ViewMut<usize>, ViewMut<u32>)| {
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
        })
        .unwrap();

    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete(entity1));
        })
        .unwrap();
}
