use super::grouping_fixtures::A;
use crate::iter::IntoIter;
use crate::{Get, View, ViewMut, World, track};
use alloc::vec::Vec;

#[test]
fn get_respects_dynamic_tracking_and_storage_replacement() {
    let mut world = World::new_for_test();
    let id = world.add_entity((A(0),));
    {
        let mut values = world.borrow::<ViewMut<'_, A>>().unwrap();
        (&mut values).get(id).unwrap().0 += 1;
        assert!(values.modification_data.is_empty());
        values.track_modification();
        assert_eq!((&mut values).get(id).unwrap().0, 1);
        assert_eq!(values.modification_data[0].get(), 0);
        (&mut values).get(id).unwrap().0 += 1;
    }
    {
        let values = world.borrow::<View<'_, A, track::Modification>>().unwrap();
        assert_eq!(values.modified().iter().ids().collect::<Vec<_>>(), [id]);
    }

    let mut values = world
        .borrow::<ViewMut<'_, A, track::Modification>>()
        .unwrap();
    *values.sparse_set = crate::sparse_set::SparseSet::new();
    let current = values.current;
    let _ = values.sparse_set.insert(id, A(5), current);
    (&mut values).get(id).unwrap().0 += 1;
    assert_eq!(values.get(id).unwrap().0, 6);
    assert!(values.modification_data.is_empty());
}
