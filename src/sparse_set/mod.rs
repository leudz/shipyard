mod pack_info;
pub mod sort;
mod view;
mod view_add_entity;

use crate::storage::EntityId;
pub(crate) use pack_info::{LoosePack, Pack, PackInfo, TightPack, UpdatePack};
pub(crate) use view::RawViewMut;
pub use view::{View, ViewMut};
pub(crate) use view_add_entity::ViewAddEntity;

// A sparse array is a data structure with 2 vectors: one sparse, the other dense.
// Only usize can be added. On insertion, the number is pushed into the dense vector
// and sparse[number] is set to dense.len() - 1.
// For all number present in the sparse array, dense[sparse[number]] == number.
// For all other values if set sparse[number] will have any value left there
// and if set dense[sparse[number]] != number.
// We can't be limited to store solely integers, this is why there is a third vector.
// It mimics the dense vector in regard to insertion/deletion.
pub struct SparseSet<T> {
    sparse: Vec<usize>,
    dense: Vec<EntityId>,
    data: Vec<T>,
    pub(crate) pack_info: PackInfo<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        SparseSet {
            sparse: Vec::new(),
            dense: Vec::new(),
            data: Vec::new(),
            pack_info: Default::default(),
        }
    }
}

impl<T: 'static> SparseSet<T> {
    /// Returns true if the sparse array contains data at this index.
    pub(crate) fn contains(&self, entity: EntityId) -> bool {
        self.view().contains(entity)
    }
    /// Returns a reference to the element at this index if present.
    pub(crate) fn get(&self, entity: EntityId) -> Option<&T> {
        if self.contains(entity) {
            Some(unsafe {
                self.data
                    .get_unchecked(*self.sparse.get_unchecked(entity.index()))
            })
        } else {
            None
        }
    }
    /// Removes and returns the element at index if present.
    pub(crate) fn remove(&mut self, entity: EntityId) -> Option<T> {
        self.view_mut().remove(entity)
    }
    /// Returns the number of element present in the sparse array.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub(crate) fn view(&self) -> View<T> {
        View {
            sparse: &self.sparse,
            dense: &self.dense,
            data: &self.data,
            pack_info: &self.pack_info,
        }
    }
    pub(crate) fn view_mut(&mut self) -> ViewMut<T> {
        ViewMut {
            sparse: &mut self.sparse,
            dense: &mut self.dense,
            data: &mut self.data,
            pack_info: &mut self.pack_info,
        }
    }
    //          ▼ old end of pack
    //              ▼ new end of pack
    // [_ _ _ _ | _ | _ _ _ _ _]
    //            ▲       ▼
    //            ---------
    //              pack
    pub(crate) fn pack(&mut self, entity: EntityId) {
        self.view_mut().pack(entity)
    }
    pub(crate) fn clone_indices(&self) -> Vec<EntityId> {
        self.dense.clone()
    }
    pub(crate) fn unpack(&mut self, entity: EntityId) {
        self.view_mut().unpack(entity)
    }
    /// Place the unique component in the storage.
    /// The storage has to be completely empty.
    pub(crate) fn insert_unique(&mut self, component: T) {
        assert!(self.sparse.is_empty() && self.dense.is_empty() && self.data.is_empty());
        self.data.push(component)
    }
    /// Returns true if this storage is a unique storage.
    pub(crate) fn is_unique(&self) -> bool {
        self.view().is_unique()
    }
}

impl<T: 'static> std::ops::Index<EntityId> for SparseSet<T> {
    type Output = T;
    fn index(&self, index: EntityId) -> &Self::Output {
        self.get(index).unwrap()
    }
}

#[test]
fn insert() {
    let mut array = SparseSet::default();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    assert!(array.view_mut().insert("0", entity_id).is_none());
    entity_id.set_index(1);
    assert!(array.view_mut().insert("1", entity_id).is_none());
    assert_eq!(array.len(), 2);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), Some(&"0"));
    entity_id.set_index(1);
    assert_eq!(array.get(entity_id), Some(&"1"));
    entity_id.set_index(5);
    assert!(array.view_mut().insert("5", entity_id).is_none());
    assert_eq!(array.view_mut().get_mut(entity_id), Some(&mut "5"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(6);
    assert_eq!(array.get(entity_id), None);
    assert!(array.view_mut().insert("6", entity_id).is_none());
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(6);
    assert_eq!(array.view_mut().get_mut(entity_id), Some(&mut "6"));
    entity_id.set_index(4);
    assert_eq!(array.get(entity_id), None);
}
#[test]
fn remove() {
    let mut array = SparseSet::default();
    let mut entity_id = EntityId::zero();
    entity_id.set_index(0);
    array.view_mut().insert("0", entity_id);
    entity_id.set_index(5);
    array.view_mut().insert("5", entity_id);
    entity_id.set_index(10);
    array.view_mut().insert("10", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.remove(entity_id), Some("0"));
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"10"));
    assert_eq!(array.remove(entity_id), Some("10"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 1);
    entity_id.set_index(3);
    array.view_mut().insert("3", entity_id);
    entity_id.set_index(10);
    array.view_mut().insert("100", entity_id);
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), Some(&"3"));
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    entity_id.set_index(3);
    assert_eq!(array.remove(entity_id), Some("3"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), Some(&"100"));
    assert_eq!(array.remove(entity_id), Some("100"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), Some(&"5"));
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.remove(entity_id), Some("5"));
    entity_id.set_index(0);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(3);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(5);
    assert_eq!(array.get(entity_id), None);
    entity_id.set_index(10);
    assert_eq!(array.get(entity_id), None);
    assert_eq!(array.len(), 0);
}
