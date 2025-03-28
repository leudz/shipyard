use shipyard::sparse_set::SparseSet;
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

#[test]
fn simple() {
    let world = World::new();

    world.run(|mut all_storages: AllStoragesViewMut| {
        let (entity0, entity1, entity2, entity3) = all_storages.run(
            |mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>, mut usizes: ViewMut<USIZE>| {
                (
                    entities.add_entity(&mut u32s, U32(0)),
                    entities.add_entity((), ()),
                    entities.add_entity(&mut usizes, USIZE(1)),
                    entities.add_entity((&mut u32s, &mut usizes), (U32(2), USIZE(3))),
                )
            },
        );

        all_storages.delete_any::<SparseSet<U32>>();

        all_storages.run(|entities: EntitiesView| {
            assert!(!entities.is_alive(entity0));
            assert!(entities.is_alive(entity1));
            assert!(entities.is_alive(entity2));
            assert!(!entities.is_alive(entity3));
        });
    });
}
