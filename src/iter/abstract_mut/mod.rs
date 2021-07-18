mod inserted;
mod inserted_or_modified;
mod modified;
mod not;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindowMut, SparseSet};
use crate::track;

pub trait AbstractMut {
    type Out;
    type Index;

    unsafe fn get_data(&self, index: usize) -> Self::Out;
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out;
    fn indices_of(&self, entity_id: EntityId, index: usize, mask: u16) -> Option<Self::Index>;
    unsafe fn indices_of_unchecked(
        &self,
        entity_id: EntityId,
        index: usize,
        mask: u16,
    ) -> Self::Index;
    unsafe fn get_id(&self, index: usize) -> EntityId;
}

impl<'tmp, T: Component> AbstractMut for &'tmp SparseSet<T, T::Tracking> {
    type Out = &'tmp T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        self.data.get_unchecked(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        self.data.get_unchecked(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.get_unchecked(index)
    }
}

impl<'tmp, T: Component<Tracking = track::Nothing>> AbstractMut
    for FullRawWindowMut<'tmp, T, track::Nothing>
{
    type Out = &'tmp mut T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
}

impl<'tmp, T: Component<Tracking = track::Insertion>> AbstractMut
    for FullRawWindowMut<'tmp, T, track::Insertion>
{
    type Out = &'tmp mut T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
}

impl<'tmp, T: Component<Tracking = track::Removal>> AbstractMut
    for FullRawWindowMut<'tmp, T, track::Removal>
{
    type Out = &'tmp mut T;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        &mut *self.data.add(index)
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
}

impl<'tmp, T: Component<Tracking = track::Modification>> AbstractMut
    for FullRawWindowMut<'tmp, T, track::Modification>
{
    type Out = Mut<'tmp, T>;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        Mut {
            flag: Some(&mut *self.dense.add(index)),
            data: &mut *self.data.add(index),
        }
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        Mut {
            flag: Some(&mut *self.dense.add(index)),
            data: &mut *self.data.add(index),
        }
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
}

impl<'tmp, T: Component<Tracking = track::All>> AbstractMut
    for FullRawWindowMut<'tmp, T, track::All>
{
    type Out = Mut<'tmp, T>;
    type Index = usize;

    #[inline]
    unsafe fn get_data(&self, index: usize) -> Self::Out {
        if self.is_tracking_modification && !(*self.dense.add(index)).is_inserted() {
            Mut {
                flag: Some(&mut *self.dense.add(index)),
                data: &mut *self.data.add(index),
            }
        } else {
            Mut {
                flag: None,
                data: &mut *self.data.add(index),
            }
        }
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        if self.is_tracking_modification && !(*self.dense.add(index)).is_inserted() {
            Mut {
                flag: Some(&mut *self.dense.add(index)),
                data: &mut *self.data.add(index),
            }
        } else {
            Mut {
                flag: None,
                data: &mut *self.data.add(index),
            }
        }
    }
    #[inline]
    fn indices_of(&self, entity_id: EntityId, _: usize, _: u16) -> Option<Self::Index> {
        self.index_of(entity_id)
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, entity_id: EntityId, _: usize, _: u16) -> Self::Index {
        self.index_of_unchecked(entity_id)
    }
    #[inline]
    unsafe fn get_id(&self, index: usize) -> EntityId {
        *self.dense.add(index)
    }
}

macro_rules! impl_abstract_mut {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: AbstractMut),+> AbstractMut for ($($type,)+) where $(<$type as AbstractMut>::Index: From<usize>),+ {
            type Out = ($(<$type as AbstractMut>::Out,)+);
            type Index = ($(<$type as AbstractMut>::Index,)+);

            #[inline]
            unsafe fn get_data(&self, index: usize) -> Self::Out {
                ($(self.$index.get_data(index),)+)
            }
            #[inline]
            unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
                ($(self.$index.get_datas(index.$index),)+)
            }
            #[inline]
            fn indices_of(&self, entity_id: EntityId, index: usize, mask: u16) -> Option<Self::Index> {
                Some(($({
                    if mask & (1 << $index) != 0 {
                        index.into()
                    } else {
                        self.$index.indices_of(entity_id, index, mask)?
                    }
                },)+))
            }
            #[inline]
            unsafe fn indices_of_unchecked(&self, entity: EntityId, index: usize, mask: u16) -> Self::Index {
                ($({
                    if mask & (1 << $index) != 0 {
                        index.into()
                    } else {
                        self.$index.indices_of_unchecked(entity, index, mask)
                    }
                },)+)
            }
            #[inline]
            unsafe fn get_id(&self, index: usize) -> EntityId {
                self.0.get_id(index)
            }
        }
    }
}

macro_rules! abstract_mut {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_abstract_mut![$(($type, $index))*];
        abstract_mut![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_abstract_mut![$(($type, $index))*];
    }
}

abstract_mut![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
