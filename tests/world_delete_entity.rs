use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let mut world = World::new();

    let entity1 = world.add_entity((0usize, 1u32));
    let entity2 = world.add_entity((2usize, 3u32));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        (&u32s).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<u32>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn update() {
    let mut world = World::new();

    world.borrow::<ViewMut<usize>>().unwrap().update_pack();

    let entity1 = world.add_entity((0usize,));
    let entity2 = world.add_entity((2usize,));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

    let mut usizes = world.borrow::<ViewMut<usize>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.deleted().len(), 1);
    assert_eq!(usizes.take_deleted(), vec![(entity1, 0)]);
    assert_eq!(usizes.removed().len(), 0);
}
