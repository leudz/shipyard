use crate::entity_id::EntityId;
use crate::ViewMut;
use core::iter::{Copied, DoubleEndedIterator, ExactSizeIterator, FusedIterator, Iterator};
use core::slice::Iter;

/// Iterator over newly bulk added entities.
///
/// Obtained from [`World::bulk_add_entity`], [`AllStorages::bulk_add_entity`] and [`Entities::bulk_add_entity`].
///
/// [`World::bulk_add_entity`]: crate::World::bulk_add_entity()
/// [`AllStorages::bulk_add_entity`]: crate::AllStorages::bulk_add_entity()
/// [`Entities::bulk_add_entity`]: crate::Entities#method::bulk_add_entity()
#[derive(Clone, Debug)]
pub struct BulkEntityIter<'a>(pub(crate) Copied<Iter<'a, EntityId>>);

impl<'a> Iterator for BulkEntityIter<'a> {
    type Item = EntityId;

    fn next(&mut self) -> Option<EntityId> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.fold(init, f)
    }

    fn nth(&mut self, n: usize) -> Option<EntityId> {
        self.0.nth(n)
    }

    fn last(self) -> Option<EntityId> {
        self.0.last()
    }

    fn count(self) -> usize {
        self.0.count()
    }
}

impl<'a> DoubleEndedIterator for BulkEntityIter<'a> {
    fn next_back(&mut self) -> Option<EntityId> {
        self.0.next_back()
    }

    fn rfold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.rfold(init, f)
    }
}

impl<'a> ExactSizeIterator for BulkEntityIter<'a> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> FusedIterator for BulkEntityIter<'a> {}

pub trait BulkReserve {
    /// Reserves memory for all entities in `new_entities`.
    fn bulk_reserve(&mut self, _new_entities: &[EntityId]) {}
}

impl BulkReserve for () {}

impl<T> BulkReserve for ViewMut<'_, T> {
    #[inline]
    fn bulk_reserve(&mut self, new_entities: &[EntityId]) {
        <&mut Self>::bulk_reserve(&mut &mut *self, new_entities);
    }
}

impl<T> BulkReserve for &mut ViewMut<'_, T> {
    #[inline]
    fn bulk_reserve(&mut self, new_entities: &[EntityId]) {
        if !new_entities.is_empty() {
            self.sparse_set
                .sparse
                .bulk_allocate(new_entities[0], new_entities[new_entities.len() - 1]);
            self.sparse_set.reserve(new_entities.len() - 1);
        }
    }
}

macro_rules! impl_bulk_add_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: BulkReserve,)+> BulkReserve for ($($storage,)+) {
            #[inline]
            fn bulk_reserve(&mut self, new_entities: &[EntityId]) {
                $(
                    self.$index.bulk_reserve(new_entities);
                )+
            }
        }
    }
}

macro_rules! bulk_add_component {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_storage: ident, $queue_index: tt))*) => {
        impl_bulk_add_component![$(($storage, $index))*];
        bulk_add_component![$(($storage, $index))* ($storage1, $index1); $(($queue_storage, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_bulk_add_component![$(($storage, $index))*];
    }
}

bulk_add_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
