use shipyard::prelude::*;

#[test]
fn off_by_one() {
    let world = World::new();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesMut, &mut usize)>();
    let entity0 = entities.add_entity(&mut u32s, 0);
    let entity1 = entities.add_entity(&mut u32s, 1);
    let entity2 = entities.add_entity(&mut u32s, 2);

    let window = u32s.as_window(1..);
    assert_eq!(window.len(), 2);
    assert!(window.get(entity0).is_err());
    assert_eq!(window.get(entity1).ok(), Some(&1));
    assert_eq!(window.get(entity2).ok(), Some(&2));
    let window = unsafe { window.as_window(1..) };
    assert_eq!(window.len(), 1);
    assert!(window.get(entity0).is_err());
    assert!(window.get(entity1).is_err());
    assert_eq!(window.get(entity2).ok(), Some(&2));

    let mut window = u32s.as_window_mut(1..);
    assert_eq!(window.len(), 2);
    assert!(window.get(entity0).is_err());
    assert_eq!((&mut window).get(entity1).ok(), Some(&mut 1));
    assert_eq!((&mut window).get(entity2).ok(), Some(&mut 2));
    let mut window = unsafe { window.as_window_mut(1..) };
    assert_eq!(window.len(), 1);
    assert!(window.get(entity0).is_err());
    assert!(window.get(entity1).is_err());
    assert_eq!((&mut window).get(entity2).ok(), Some(&mut 2));
}
