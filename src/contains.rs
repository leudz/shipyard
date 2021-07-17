use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::view::{View, ViewMut};

/// Checks if an entity has some components.
pub trait Contains {
    /// Returns true if all storages contains `entity`.
    fn contains(&self, entity: EntityId) -> bool;
}

impl<'a: 'b, 'b, T: Component> Contains for &'b View<'a, T> {
    fn contains(&self, entity: EntityId) -> bool {
        SparseSet::contains(&*self, entity)
    }
}

impl<'a: 'b, 'b, T: Component> Contains for &'b ViewMut<'a, T> {
    fn contains(&self, entity: EntityId) -> bool {
        SparseSet::contains(&**self, entity)
    }
}

impl<'a: 'b, 'b, T: Component> Contains for &'b mut ViewMut<'a, T> {
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

contains![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
