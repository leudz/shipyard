use super::IntoSortable;
use crate::error;
use crate::sparse_set::{EntityId, Pack, ViewMut};
use std::any::TypeId;
use std::cmp::Ordering;

pub struct UnstableSort1<'tmp, 'view: 'tmp, T>(&'tmp mut ViewMut<'view, T>);

impl<'tmp, 'view: 'tmp, T> IntoSortable for &'tmp mut ViewMut<'view, T> {
    type IntoSortable = UnstableSort1<'tmp, 'view, T>;
    fn sort(self) -> Self::IntoSortable {
        UnstableSort1(self)
    }
}

impl<'tmp, 'view: 'tmp, T> UnstableSort1<'tmp, 'view, T> {
    pub fn try_unstable(self, mut cmp: impl FnMut(&T, &T) -> Ordering) -> Result<(), error::Sort> {
        if std::mem::discriminant(&self.0.pack_info.pack) == std::mem::discriminant(&Pack::NoPack) {
            let mut transform: Vec<usize> = (0..self.0.dense.len()).collect();

            transform.sort_unstable_by(|&i, &j| {
                cmp(unsafe { self.0.data.get_unchecked(i) }, unsafe {
                    self.0.data.get_unchecked(j)
                })
            });

            let mut pos;
            for i in 0..transform.len() {
                pos = unsafe { *transform.get_unchecked(i) };
                while pos < i {
                    pos = unsafe { *transform.get_unchecked(pos) };
                }
                self.0.dense.swap(i, pos);
                self.0.data.swap(i, pos);
            }

            for i in 0..self.0.dense.len() {
                unsafe {
                    *self
                        .0
                        .sparse
                        .get_unchecked_mut(self.0.dense.get_unchecked(i).index()) = i;
                }
            }

            Ok(())
        } else {
            Err(error::Sort::MissingPackStorage)
        }
    }
    pub fn unstable(self, cmp: impl FnMut(&T, &T) -> Ordering) {
        self.try_unstable(cmp).unwrap()
    }
}

macro_rules! impl_unstable_sort {
    ($sort: ident; $(($type: ident, $index: tt))+) => {
        pub struct $sort<'tmp, 'view: 'tmp, $($type),+>($(&'tmp mut ViewMut<'view, $type>,)+);

        impl<'tmp, 'view: 'tmp, $($type),+> IntoSortable for ($(&'tmp mut ViewMut<'view, $type>,)+) {
            type IntoSortable = $sort<'tmp, 'view, $($type,)+>;

            fn sort(self) -> Self::IntoSortable {
                $sort($(self.$index,)+)
            }
        }

        impl<'tmp, 'view: 'tmp, $($type: 'static),+> $sort<'tmp, 'view, $($type),+> {
            pub fn try_unstable<Cmp: FnMut(($(&$type,)+), ($(&$type,)+)) -> Ordering>(self, mut cmp: Cmp) -> Result<(), error::Sort> {
                enum PackSort {
                    Tight(usize),
                    Loose(usize),
                    None,
                }

                let mut type_ids = [$(TypeId::of::<$type>()),+];
                type_ids.sort_unstable();
                let mut pack_sort = PackSort::None;

                $({
                    if let PackSort::None = pack_sort {
                        match &self.$index.pack_info.pack {
                            Pack::Tight(pack) => {
                                if let Ok(types) = pack.check_types(&type_ids) {
                                    if types.len() == type_ids.len() {
                                        pack_sort = PackSort::Tight(pack.len);
                                    } else if types.len() < type_ids.len() {
                                        return Err(error::Sort::TooManyStorages);
                                    } else {
                                        return Err(error::Sort::MissingPackStorage);
                                    }
                                } else {
                                    return Err(error::Sort::MissingPackStorage);
                                }
                            }
                            Pack::Loose(pack) => {
                                if pack.check_all_types(&type_ids).is_ok() {
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
                            Pack::Update(_) => return Err(error::Sort::TooManyStorages),
                            Pack::NoPack => return Err(error::Sort::TooManyStorages),
                        }
                    }
                })+

                match pack_sort {
                    PackSort::Tight(len) => {
                        let mut transform: Vec<usize> = (0..len).collect();

                        transform.sort_unstable_by(|&i, &j| cmp(
                            ($(unsafe {self.$index.data.get_unchecked(i)},)+),
                            ($(unsafe {self.$index.data.get_unchecked(j)},)+),
                        ));

                        let mut pos;
                        $(
                            for i in 0..transform.len() {
                                pos = unsafe {*transform.get_unchecked(i)};
                                while pos < i {
                                    pos = unsafe { *transform.get_unchecked(pos) };
                                }
                                self.$index.dense.swap(i, pos);
                                self.$index.data.swap(i, pos);
                            }

                            for i in 0..self.$index.dense.len() {
                                unsafe {
                                    *self.$index.sparse.get_unchecked_mut(self.0.dense.get_unchecked(i).index()) = i;
                                }
                            }
                        )*

                        Ok(())
                    }
                    PackSort::Loose(len) => {
                        let mut dense: &[EntityId] = &[];
                        let mut packed = 0;
                        $(
                            if self.$index.pack_info.pack.is_loose() {
                                dense = &self.$index.dense;
                                packed |= 1 << $index;
                            }
                        )+

                        let mut transform: Vec<usize> = (0..len).collect();

                        transform.sort_unstable_by(|&i, &j| cmp(
                            ($(
                                unsafe {
                                    if packed & 1 << $index != 0 {
                                        self.$index.data.get_unchecked(i)
                                    } else {
                                        self.$index.data.get_unchecked(*self.$index.sparse.get_unchecked(dense.get_unchecked(i).index()))
                                    }
                                }
                            ,)+),
                            ($(
                                unsafe {
                                    if packed & 1 << $index != 0 {
                                        self.$index.data.get_unchecked(j)
                                    } else {
                                        self.$index.data.get_unchecked(*self.$index.sparse.get_unchecked(dense.get_unchecked(j).index()))
                                    }
                                }
                            ,)+)
                        ));

                        let mut pos;
                        $(
                            for i in 0..transform.len() {
                                pos = unsafe {*transform.get_unchecked(i)};
                                while pos < i {
                                    pos = unsafe { *transform.get_unchecked(pos) };
                                }
                                self.$index.dense.swap(i, pos);
                                self.$index.data.swap(i, pos);
                            }

                            for i in 0..self.$index.dense.len() {
                                unsafe {
                                    *self.$index.sparse.get_unchecked_mut(self.0.dense.get_unchecked(i).index()) = i;
                                }
                            }
                        )*

                        Ok(())
                    }
                    PackSort::None => unreachable!(),
                }
            }
            pub fn unstable<Cmp: FnMut(($(&$type,)+), ($(&$type,)+)) -> Ordering>(self, cmp: Cmp) {
                self.try_unstable(cmp).unwrap()
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

unstable_sort![;UnstableSort2 UnstableSort3 UnstableSort4 UnstableSort5 UnstableSort6 UnstableSort7 UnstableSort8 UnstableSort9 UnstableSort10;(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];

#[test]
fn unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::default();

    for i in (0..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i);
        array.view_mut().insert(i, entity_id);
    }

    array
        .view_mut()
        .sort()
        .unstable(|x: &u64, y: &u64| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i);
        assert_eq!(array.get(entity_id), Some(&i));
    }
}

#[test]
fn partially_sorted_unstable_sort() {
    let mut array = crate::sparse_set::SparseSet::default();

    for i in 0..20 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(i);
        assert!(array.view_mut().insert(i, entity_id).is_none());
    }
    for i in (20..100).rev() {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert!(array.view_mut().insert(i, entity_id).is_none());
    }

    array
        .view_mut()
        .sort()
        .unstable(|x: &u64, y: &u64| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(i);
        assert_eq!(array.get(entity_id), Some(&i));
    }
    for i in 20..100 {
        let mut entity_id = crate::storage::EntityId::zero();
        entity_id.set_index(100 - i + 20);
        assert_eq!(array.get(entity_id), Some(&i));
    }
}
