use super::{IntoSortable, SparseSet};
use crate::view::ViewMut;
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

macro_rules! impl_unstable_sort {
    ($sort: ident; $(($type: ident, $index: tt))+) => {
        /// Struct used to sort multiple storages.
        pub struct $sort<'tmp, $($type),+>($(&'tmp mut SparseSet<$type>,)+);

        impl<'tmp, $($type),+> IntoSortable for ($(&'tmp mut ViewMut<'_, $type>,)+) {
            type IntoSortable = $sort<'tmp, $($type,)+>;

            fn sort(self) -> Self::IntoSortable {
                $sort($(self.$index,)+)
            }
        }
    }
}

macro_rules! unstable_sort {
    ($($sort: ident)*; $sort1: ident $($queue_sort: ident)*;$(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_unstable_sort![$sort1; $(($type, $index))*];
        unstable_sort![$($sort)* $sort1; $($queue_sort)*; $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($($sort: ident)+; $sort1: ident; $(($type: ident, $index: tt))+;) => {
        impl_unstable_sort![$sort1; $(($type, $index))*];
    }
}

unstable_sort![;Sort2 Sort3 Sort4 Sort5 Sort6 Sort7 Sort8 Sort9 Sort10;(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];

#[test]
fn unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::new();

    for i in (0..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i);
        array.insert(i, entity_id);
    }

    array
        .sort()
        .unstable(|x: &u64, y: &u64| x.cmp(&y));

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
        assert!(array.insert(i, entity_id).is_none());
    }
    for i in (20..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert!(array.insert(i, entity_id).is_none());
    }

    array
        .sort()
        .unstable(|x: &u64, y: &u64| x.cmp(&y));

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
