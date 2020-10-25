use super::{IntoSortable, SparseSet};
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Struct used to sort a single storage.
pub struct Sort1<'tmp, T>(&'tmp mut SparseSet<T>);

impl<'tmp, T> IntoSortable for &'tmp mut SparseSet<T> {
    type IntoSortable = Sort1<'tmp, T>;
    fn sort(self) -> Self::IntoSortable {
        Sort1(self)
    }
}

impl<'tmp, T> Sort1<'tmp, T> {
    /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.
    pub fn unstable(self, mut cmp: impl FnMut(&T, &T) -> Ordering) {
        let mut transform: Vec<usize> = (0..self.0.dense.len()).collect();

        transform.sort_unstable_by(|&i, &j| {
            // SAFE dense and data have the same length
            cmp(unsafe { self.0.data.get_unchecked(i) }, unsafe {
                self.0.data.get_unchecked(j)
            })
        });

        let mut pos;
        for i in 0..transform.len() {
            // SAFE we're in bound
            pos = unsafe { *transform.get_unchecked(i) };
            while pos < i {
                // SAFE we're in bound
                pos = unsafe { *transform.get_unchecked(pos) };
            }
            self.0.dense.swap(i, pos);
            self.0.data.swap(i, pos);
        }

        for i in 0..self.0.dense.len() {
            let dense = self.0.dense[i];
            unsafe {
                self.0.sparse.get_mut_unchecked(dense).set_index(i as u64);
            }
        }
    }
}

#[test]
fn unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::new();

    for i in (0..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i);
        array.insert(entity_id, i);
    }

    array.sort().unstable(|x: &u64, y: &u64| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
}

#[test]
fn partially_sorted_unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::new();

    for i in 0..20 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(i);
        assert!(array.insert(entity_id, i).is_none());
    }
    for i in (20..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert!(array.insert(entity_id, i).is_none());
    }

    array.sort().unstable(|x: &u64, y: &u64| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(i);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
    for i in 20..100 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert_eq!(array.private_get(entity_id), Some(&i));
    }
}
