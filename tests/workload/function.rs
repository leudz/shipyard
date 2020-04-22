use shipyard::*;

#[test]
fn basic() {
    let world = World::new::<(u32, i16)>();
    let g = 9.81;
    world.add_workload_fn("Gravity", move |world: &World| {
        world.try_run::<(&mut u32, &i16), _, _>(|(u32s, i16s)| {
            (u32s, i16s).iter().for_each(|(x, y)| {
                *x += (*y as f32 * g) as u32;
            });
        })
    });
    world.run::<(EntitiesMut, &mut u32, &mut i16), _, _>(|(mut entities, mut u32s, mut i16s)| {
        entities.add_entity((&mut u32s, &mut i16s), (0, 1));
        entities.add_entity(&mut i16s, 3);
        entities.add_entity((&mut u32s, &mut i16s), (4, 5));
        entities.add_entity(&mut u32s, 6);
    });
    world.run_default();
    world.run::<&u32, _, _>(|u32s| {
        let mut iter = u32s.iter();
        assert_eq!(iter.next(), Some(&9));
        assert_eq!(iter.next(), Some(&53));
        assert_eq!(iter.next(), Some(&6));
        assert_eq!(iter.next(), None);
    });
}
