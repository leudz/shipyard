use shipyard::prelude::*;

#[test]
fn simple_borrow() {
    let world = World::new();

    let u32s = world.borrow::<&u32>();
    assert_eq!(u32s.len(), 0);
}

#[test]
#[should_panic]
fn invalid_borrow() {
    let world = World::new();

    let _u32s = world.borrow::<&mut u32>();
    world.borrow::<&mut u32>();
}

#[test]
fn double_borrow() {
    let world = World::new();

    let u32s = world.borrow::<&mut u32>();
    drop(u32s);
    world.borrow::<&mut u32>();
}
