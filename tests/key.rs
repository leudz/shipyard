use shipyard::*;

#[allow(unused)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[cfg(feature = "parallel")]
struct Tracked(usize);
#[cfg(feature = "parallel")]
impl Component for Tracked {
    type Tracking = track::All;
}

#[test]
fn key_equality() {
    let world = World::new();

    //create 3 entities
    let (e0, e1, e2) = world.run(
        |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<USIZE>)| {
            (
                entities.add_entity(&mut usizes, USIZE(0)),
                entities.add_entity(&mut usizes, USIZE(1)),
                entities.add_entity(&mut usizes, USIZE(2)),
            )
        },
    );

    //add a component to e1
    world.run(
        |(ref mut entities, ref mut u32s): (EntitiesViewMut, ViewMut<U32>)| {
            entities.add_component(e1, u32s, U32(42));
        },
    );

    //confirm that the entity keys have not changed for usizes storage
    world.run(|usizes: View<USIZE>| {
        //sanity check
        assert_eq!((&usizes).iter().with_id().count(), 3);

        let keys: Vec<EntityId> =
            (&usizes)
                .iter()
                .with_id()
                .map(|(entity, _)| entity)
                .fold(Vec::new(), |mut vec, x| {
                    vec.push(x);
                    vec
                });

        assert_eq!(keys, vec![e0, e1, e2]);
    });

    //confirm that the entity id for (USIZE) is the same as (USIZE, U32)
    //in other words that the entity itself did not somehow change from adding a component
    world.run(|(usizes, u32s): (View<USIZE>, View<U32>)| {
        //sanity check
        assert_eq!((&usizes, &u32s).iter().with_id().count(), 1);

        let (entity, (_, _)) = (&usizes, &u32s).iter().with_id().find(|_| true).unwrap();
        assert_eq!(entity, e1);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_with_id_keeps_entity_alignment() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use rayon::prelude::*;

    let mut world = World::new();
    let ids = (0..300)
        .map(|i| {
            if i % 2 == 0 {
                world.add_entity((USIZE(i), U32(i as u32), Tracked(i)))
            } else {
                world.add_entity((USIZE(i), Tracked(i)))
            }
        })
        .collect::<Vec<_>>();
    world.clear_all_inserted_and_modified();
    world.run(|_tracked: ViewMut<Tracked, track::All>| {});

    world.run(|usizes: View<USIZE>| {
        let mut actual = (&usizes)
            .par_iter()
            .with_id()
            .map(|(id, value)| (id, value.0))
            .collect::<Vec<_>>();
        actual.sort_unstable_by_key(|(_, value)| *value);

        assert_eq!(actual, ids.iter().copied().zip(0..300).collect::<Vec<_>>());
    });

    world.run(|(usizes, u32s): (View<USIZE>, View<U32>)| {
        (&usizes, &u32s)
            .par_iter()
            .with_id()
            .for_each(|(id, (usize, u32))| {
                assert_eq!(id, ids[usize.0]);
                assert_eq!(u32.0, usize.0 as u32);
            });
    });

    world.run(|mut usizes: ViewMut<USIZE>| {
        (&mut usizes).par_iter().with_id().for_each(|(id, value)| {
            assert_eq!(id, ids[value.0]);
            value.0 += 1;
        });
    });

    world.run(|mut tracked: ViewMut<Tracked, track::All>| {
        for &index in &[5usize, 70, 200, 299] {
            let mut value = (&mut tracked).get(ids[index]).unwrap();
            value.0 = index;
        }
    });

    world.run(|tracked: ViewMut<Tracked, track::All>| {
        let hits = AtomicUsize::new(0);

        tracked
            .modified()
            .par_iter()
            .with_id()
            .for_each(|(id, value)| {
                assert_eq!(id, ids[value.0]);
                hits.fetch_add(1, Ordering::Relaxed);
            });

        assert_eq!(hits.load(Ordering::Relaxed), 4);
    });
}
