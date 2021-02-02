use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let mut world = World::new();

    let entity1 = world.add_entity((0usize, 1u32));
    let entity2 = world.add_entity((2usize, 3u32));

    world.delete_component::<(usize,)>(entity1);

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(u32s.get(entity1), Ok(&1));
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn update() {
    let mut world = World::new();

    world.borrow::<ViewMut<usize>>().unwrap().track_all();

    let entity1 = world.add_entity((0usize,));
    let entity2 = world.add_entity((2usize,));

    world.delete_component::<(usize,)>(entity1);

    world
        .run(|mut usizes: ViewMut<usize>| {
            assert_eq!(
                usizes.get(entity1),
                Err(error::MissingComponent {
                    id: entity1,
                    name: type_name::<usize>(),
                })
            );
            assert_eq!(usizes.get(entity2), Ok(&2));
            assert_eq!(usizes.len(), 1);
            assert_eq!(usizes.inserted().iter().count(), 1);
            assert_eq!(usizes.modified().iter().count(), 0);
            assert_eq!(usizes.deleted(), &[(entity1, 0)]);

            let mut iter = usizes.removed_or_deleted();
            assert_eq!(iter.next(), Some(entity1));
            assert_eq!(iter.next(), None);

            drop(iter);

            assert_eq!(usizes.take_deleted(), vec![(entity1, 0)]);
        })
        .unwrap();
}

#[test]
fn strip() {
    let mut world = World::new();

    let entity1 = world.add_entity((0usize, 1u32));
    let entity2 = world.add_entity((2usize, 3u32));

    world.strip(entity1);

    world
        .run(|usizes: View<usize>, u32s: View<u32>| {
            assert_eq!(
                usizes.get(entity1).err(),
                Some(error::MissingComponent {
                    id: entity1,
                    name: type_name::<usize>(),
                })
            );
            assert_eq!(
                u32s.get(entity1).err(),
                Some(error::MissingComponent {
                    id: entity1,
                    name: type_name::<u32>(),
                })
            );
            assert_eq!(usizes.get(entity2), Ok(&2));
            assert_eq!(u32s.get(entity2), Ok(&3));
        })
        .unwrap();

    assert!(world.delete_entity(entity1));
}
