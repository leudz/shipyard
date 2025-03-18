use crate::component::Component;
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
pub struct BulkEntityIter<'a> {
    pub(crate) iter: Copied<Iter<'a, EntityId>>,
    pub(crate) slice: &'a [EntityId],
}

impl BulkEntityIter<'_> {
    /// [`EntityId`] slice of the newly bulk added entities.  
    pub fn as_slice(&self) -> &[EntityId] {
        self.slice
    }
}

impl<'a> Iterator for BulkEntityIter<'a> {
    type Item = EntityId;

    fn next(&mut self) -> Option<EntityId> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, f)
    }

    fn nth(&mut self, n: usize) -> Option<EntityId> {
        self.iter.nth(n)
    }

    fn last(self) -> Option<EntityId> {
        self.iter.last()
    }

    fn count(self) -> usize {
        self.iter.count()
    }
}

impl<'a> DoubleEndedIterator for BulkEntityIter<'a> {
    fn next_back(&mut self) -> Option<EntityId> {
        self.iter.next_back()
    }

    fn rfold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.rfold(init, f)
    }
}

impl<'a> ExactSizeIterator for BulkEntityIter<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a> FusedIterator for BulkEntityIter<'a> {}

/// Reserves memory for a set of entities.
pub trait BulkReserve {
    /// Reserves memory for all entities in `new_entities`.
    #[allow(unused_variables)]
    fn bulk_reserve(&mut self, new_entities: &[EntityId]) {}
}

impl BulkReserve for () {}

impl<T: Component, TRACK> BulkReserve for ViewMut<'_, T, TRACK> {
    #[inline]
    fn bulk_reserve(&mut self, new_entities: &[EntityId]) {
        <&mut Self>::bulk_reserve(&mut &mut *self, new_entities);
    }
}

impl<T: Component, TRACK> BulkReserve for &mut ViewMut<'_, T, TRACK> {
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

#[cfg(not(feature = "extended_tuple"))]
bulk_add_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
bulk_add_component![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
