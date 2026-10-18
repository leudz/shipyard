use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    let mut world = World::new();

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
    let mut world = World::new();
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

/// Asserts that `retain_mut` over a grouped storage keeps exactly the accepted components with
/// the values it wrote, and that the kept pairs are yielded, before and after a regroup.
#[test]
fn grouped() {
    #[derive(PartialEq, Eq, Debug)]
    struct Other(u32);
    impl Component for Other {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();
    let entities: Vec<EntityId> = (0..6)
        .map(|i| world.add_entity((U32(i), Other(i))))
        .collect();
    world.group::<(U32, Other)>();

    world.retain_mut::<U32>(|_, mut i| {
        i.0 *= 2;

        i.0 % 3 != 0
    });

    assert!(world.get::<&U32>(entities[3]).is_err());
    let pairs = |world: &World| {
        world.run(|(u32s, others): (View<U32>, View<Other>)| {
            let mut pairs: Vec<_> = (&u32s, &others)
                .iter()
                .with_id()
                .map(|(id, (u32, other))| (id, u32.0, other.0))
                .collect();
            pairs.sort_unstable();
            pairs
        })
    };
    let expected = [1, 2, 4, 5].map(|i| (entities[i], 2 * i as u32, i as u32));
    assert_eq!(pairs(&world), expected);

    world.regroup();
    assert_eq!(pairs(&world), expected);
}
