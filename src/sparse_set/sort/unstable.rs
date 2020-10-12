use super::{IntoSortable, SparseSet};
use crate::error;
use crate::sparse_set::{EntityId, Pack};
use crate::type_id::TypeId;
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
    pub fn try_unstable(self, mut cmp: impl FnMut(&T, &T) -> Ordering) -> Result<(), error::Sort> {
        if core::mem::discriminant(&self.0.metadata.pack) == core::mem::discriminant(&Pack::None) {
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

            Ok(())
        } else {
            Err(error::Sort::MissingPackStorage)
        }
    }
    /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.  
    /// Unwraps errors.
    #[cfg(feature = "panic")]
    #[track_caller]
    pub fn unstable(self, cmp: impl FnMut(&T, &T) -> Ordering) {
        match self.try_unstable(cmp) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
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

        impl<'tmp, 'view, $($type: 'static),+> $sort<'tmp, $($type),+> {
            /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.
            pub fn try_unstable<Cmp: FnMut(($(&$type,)+), ($(&$type,)+)) -> Ordering>(self, mut cmp: Cmp) -> Result<(), error::Sort> {
                enum PackSort {
                    Tight(usize),
                    Loose(usize),
                    None,
                }

                let mut type_ids = [$(TypeId::of::<SparseSet<$type>>()),+];
                type_ids.sort_unstable();
                let mut pack_sort = PackSort::None;

                $({
                    if let PackSort::None = pack_sort {
                        match &self.$index.metadata.pack {
                            Pack::Tight(pack) => {
                                if pack.is_packable(&type_ids) {
                                    if pack.types.len() == type_ids.len() {
                                        pack_sort = PackSort::Tight(pack.len);
                                    } else if pack.types.len() < type_ids.len() {
                                        return Err(error::Sort::TooManyStorages);
                                    } else {
                                        return Err(error::Sort::MissingPackStorage);
                                    }
                                } else {
                                    return Err(error::Sort::MissingPackStorage);
                                }
                            }
                            Pack::Loose(pack) => {
                                if pack.is_packable(&type_ids) {
                                    if pack.tight_types.len() + pack.loose_types.len() == type_ids.len() {
                                        pack_sort = PackSort::Loose(pack.len);
                                    } else if pack.tight_types.len() + pack.loose_types.len() < type_ids.len() {
                                        return Err(error::Sort::TooManyStorages);
                                    } else {
                                        return Err(error::Sort::MissingPackStorage);
                                    }
                                } else {
                                    return Err(error::Sort::MissingPackStorage);
                                }
                            }
                            Pack::None => return Err(error::Sort::TooManyStorages),
                        }
                    }
                })+

                match pack_sort {
                    PackSort::Tight(len) => {
                        let mut transform: Vec<usize> = (0..len).collect();

                        // SAFE dense and data have the same length
                        transform.sort_unstable_by(|&i, &j| cmp(
                            ($(unsafe {self.$index.data.get_unchecked(i)},)+),
                            ($(unsafe {self.$index.data.get_unchecked(j)},)+),
                        ));

                        let mut pos;
                        $(
                            for i in 0..transform.len() {
                                // SAFE we're in bound
                                pos = unsafe {*transform.get_unchecked(i)};
                                while pos < i {
                                    // SAFE we're in bound
                                    pos = unsafe { *transform.get_unchecked(pos) };
                                }
                                self.$index.dense.swap(i, pos);
                                self.$index.data.swap(i, pos);
                            }

                            for i in 0..self.$index.dense.len() {
                                unsafe {
                                    // SAFE i is in bound
                                    let dense = *self.0.dense.get_unchecked(i);
                                    // SAFE dense can always index into sparse
                                    self.$index.sparse.get_mut_unchecked(dense).set_index(i as u64);
                                }
                            }
                        )*

                        Ok(())
                    }
                    PackSort::Loose(len) => {
                        let mut dense: &[EntityId] = &[];
                        let mut packed = 0;
                        $(
                            if let Pack::Loose(_) = &self.$index.metadata.pack {
                                dense = &self.$index.dense;
                                packed |= 1 << $index;
                            }
                        )+

                        let mut transform: Vec<usize> = (0..len).collect();

                        transform.sort_unstable_by(|&i, &j| cmp(
                            ($(
                                unsafe {
                                    if packed & (1 << $index) != 0 {
                                        // SAFE i is in bound
                                        self.$index.data.get_unchecked(i)
                                    } else {
                                        // SAFE i is in bound
                                        let id = *dense.get_unchecked(i);
                                        // SAFE dense can always index into sparse
                                        let index = self.$index.sparse.get_unchecked(id);
                                        // SAFE sparse can always index into data
                                        self.$index.data.get_unchecked(index.uindex())
                                    }
                                }
                            ,)+),
                            ($(
                                unsafe {
                                    if packed & (1 << $index) != 0 {
                                        // SAFE j is in bound
                                        self.$index.data.get_unchecked(j)
                                    } else {
                                        // SAFE j is in bound
                                        let id = *dense.get_unchecked(j);
                                        // SAFE dense can always index into sparse
                                        let index = self.$index.sparse.get_unchecked(id);
                                        // SAFE sparse can always index into data
                                        self.$index.data.get_unchecked(index.uindex())
                                    }
                                }
                            ,)+)
                        ));

                        let mut pos;
                        $(
                            for i in 0..transform.len() {
                                // SAFE i is in bound
                                pos = unsafe {*transform.get_unchecked(i)};
                                while pos < i {
                                    // SAFE pos is in bound
                                    pos = unsafe { *transform.get_unchecked(pos) };
                                }
                                self.$index.dense.swap(i, pos);
                                self.$index.data.swap(i, pos);
                            }

                            for i in 0..self.$index.dense.len() {
                                unsafe {
                                    // SAFE i is in bound
                                    let dense = *self.0.dense.get_unchecked(i);
                                    // SAFE dense can always index into sparse
                                    self.$index.sparse.get_mut_unchecked(dense).set_index(i as u64);
                                }
                            }
                        )*

                        Ok(())
                    }
                    PackSort::None => unreachable!(),
                }
            }
            /// Sorts the storage(s) using an unstable algorithm, it may reorder equal components.
            /// Unwraps errors.
            #[cfg(feature = "panic")]
            #[track_caller]
            pub fn unstable<Cmp: FnMut(($(&$type,)+), ($(&$type,)+)) -> Ordering>(self, cmp: Cmp) {
                match self.try_unstable(cmp) {
                    Ok(_) => (),
                    Err(err) => panic!("{:?}", err),
                }
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
        .try_unstable(|x: &u64, y: &u64| x.cmp(&y))
        .unwrap();

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
        .try_unstable(|x: &u64, y: &u64| x.cmp(&y))
        .unwrap();

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
