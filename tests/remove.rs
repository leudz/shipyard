use core::any::type_name;
use shipyard::error;
use shipyard::iterators;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) =
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(0usize));
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
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).tight_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
    assert_eq!(component, (Some(0usize),));
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
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>();

    (&mut usizes, &mut u32s).loose_pack();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
    assert_eq!(component, (Some(0usize),));
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
        world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>();

    (&mut usizes, &mut u64s).tight_pack();
    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));
    entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
    let component = Remove::<(u32,)>::remove((&mut u32s, &mut usizes, &mut u64s), entity1);
    assert_eq!(component, (Some(2),));
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
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>();

    usizes.update_pack();

    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(0));
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
    assert_eq!(usizes.deleted().len(), 0);
}

#[test]
fn old_key() {
    let world = World::new();

    let entity = world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32))
        },
    );

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.delete(entity);
    });

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            let (old_usize, old_u32) =
                Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity);
            assert!(old_usize.is_none() && old_u32.is_none());
        },
    );
}

#[test]
fn newer_key() {
    let world = World::new();

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)| {
            let entity = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));

            entities.delete_unchecked(entity);
            assert_eq!(usizes.len(), 1);
            assert_eq!(u32s.len(), 1);
            let new_entity = entities.add_entity((), ());
            let (old_usize, old_u32) =
                Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), new_entity);
            assert!(old_usize.is_none() && old_u32.is_none());
            assert_eq!(usizes.len(), 0);
            assert_eq!(u32s.len(), 0);
        },
    );
}
