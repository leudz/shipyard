use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {}

#[test]
fn no_pack() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let eid0 = world.add_entity(U32(0));
    let eid1 = world.add_entity(U32(1));

    world.retain_mut::<U32>(|_, mut i| {
        i.0 *= 2;

        i.0 != 0
    });

    assert!(world.get::<&U32>(eid0).is_err());
    assert_eq!(*world.get::<&U32>(eid1).unwrap(), &U32(2));
}

#[test]
fn track() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.track_all::<U32>();

    let eid0 = world.add_entity(U32(0));
    let eid1 = world.add_entity(U32(1));

    world.retain_mut::<U32>(|_, mut i| {
        i.0 *= 2;

        i.0 != 0
    });

    assert!(world.get::<&U32>(eid0).is_err());
    assert_eq!(*world.get::<&U32>(eid1).unwrap(), &U32(2));
    world.run(|v_u32: View<U32, track::All>| {
        let mut deleted = v_u32.deleted();
        assert_eq!(deleted.next(), Some((eid0, &U32(0))));
        assert!(deleted.next().is_none());

        assert!(v_u32.is_modified(eid1));
    });
}
