use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::tracking::Tracking;
use crate::views::{View, ViewMut};

/// Checks if an entity has some components.
pub trait Contains {
    /// Returns true if all storages contains `entity`.
    fn contains(&self, entity: EntityId) -> bool;
}

impl<'a: 'b, 'b, T: Component, Track: Tracking> Contains for &'b View<'a, T, Track> {
    fn contains(&self, entity: EntityId) -> bool {
        SparseSet::contains(self, entity)
    }
}

impl<'a: 'b, 'b, T: Component, Track: Tracking> Contains for &'b ViewMut<'a, T, Track> {
    fn contains(&self, entity: EntityId) -> bool {
        SparseSet::contains(&**self, entity)
    }
}

impl<'a: 'b, 'b, T: Component, Track: Tracking> Contains for &'b mut ViewMut<'a, T, Track> {
    fn contains(&self, entity: EntityId) -> bool {
        SparseSet::contains(&**self, entity)
    }
}

macro_rules! impl_contains {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Contains),+> Contains for ($($type,)+) {
            fn contains(&self, entity: EntityId) -> bool {
                $(self.$index.contains(entity))&&+
            }
        }
    }
}

macro_rules! contains {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_contains![$(($type, $index))*];
        contains![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_contains![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
contains![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
contains![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
