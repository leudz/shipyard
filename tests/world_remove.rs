use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();

    let entity1 = world.add_entity((USIZE(0), U32(1u32)));
    let entity2 = world.add_entity((USIZE(2), U32(3u32)));

    let (component,) = world.remove::<(USIZE,)>(entity1);
    assert_eq!(component, Some(USIZE(0)));

    let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(u32s.get(entity1), Ok(&U32(1)));
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(u32s.get(entity2), Ok(&U32(3)));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity1 = world.add_entity((USIZE(0usize),));
    let entity2 = world.add_entity((USIZE(2usize),));

    let (component,) = world.remove::<(USIZE,)>(entity1);
    assert_eq!(component, Some(USIZE(0)));

    world.run(|usizes: View<USIZE, track::All>| {
        assert_eq!(
            usizes.get(entity1),
            Err(error::MissingComponent {
                id: entity1,
                name: type_name::<USIZE>(),
            })
        );
        assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
        assert_eq!(usizes.len(), 1);
        assert_eq!(usizes.inserted().iter().count(), 1);
        assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity1]);
    });

    world.remove::<(USIZE,)>(entity2);

    world.run(|usizes: View<USIZE, track::All>| {
        assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity1, entity2]);
    });
}

#[test]
fn old_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();

    let entity = world.add_entity((USIZE(0), U32(1)));
    world.delete_entity(entity);

    world.add_entity((USIZE(2), U32(3)));

    let (old_usize, old_u32) = world.remove::<(USIZE, U32)>(entity);
    assert!(old_usize.is_none() && old_u32.is_none());
}

#[test]
fn newer_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();

    let entity = world.add_entity((USIZE(0), U32(1)));

    world
        .borrow::<EntitiesViewMut>()
        .unwrap()
        .delete_unchecked(entity);

    world.run(|(usizes, u32s): (ViewMut<USIZE>, ViewMut<U32>)| {
        assert_eq!(usizes.len(), 1);
        assert_eq!(u32s.len(), 1);
    });

    let new_entity = world.add_entity(());
    let (old_usize, old_u32) = world.remove::<(USIZE, U32)>(new_entity);

    assert_eq!(old_usize, None);
    assert_eq!(old_u32, None);

    world.run(|(usizes, u32s): (ViewMut<USIZE>, ViewMut<U32>)| {
        assert_eq!(usizes.len(), 0);
        assert_eq!(u32s.len(), 0);
    });
}
