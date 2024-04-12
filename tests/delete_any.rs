use shipyard::*;

struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn simple() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.run(|mut all_storages: AllStoragesViewMut| {
        let (entity0, entity1, entity2, entity3) = all_storages.run(
            |mut entities: EntitiesViewMut, mut u64s: ViewMut<U64>, mut usizes: ViewMut<USIZE>| {
                (
                    entities.add_entity(&mut u64s, U64(0)),
                    entities.add_entity((), ()),
                    entities.add_entity(&mut usizes, USIZE(1)),
                    entities.add_entity((&mut u64s, &mut usizes), (U64(2), USIZE(3))),
                )
            },
        );

        all_storages.delete_any::<SparseSet<U64>>();

        all_storages.run(|entities: EntitiesView| {
            assert!(!entities.is_alive(entity0));
            assert!(entities.is_alive(entity1));
            assert!(entities.is_alive(entity2));
            assert!(!entities.is_alive(entity3));
        });
    });
}
