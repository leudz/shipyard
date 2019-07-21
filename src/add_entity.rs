use crate::entity::{Entities, Key};
use crate::sparse_array::{SparseArray, Write};

// `AddEntity` will store the components in a new entity
// No new storage will be created
pub trait AddEntity {
    type Component;
    /// Stores `component` in a new entity, the `Key` to this entity is returned.
    /// Multiple components can be added at the same time using a tuple.
    ///
    /// Due to current restriction, the storages and `component` have to be tuples,
    /// even for a single value. In this case use (T,).
    fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key;
}

impl AddEntity for () {
    type Component = ();
    fn add_entity(self, _: Self::Component, entities: &mut Entities) -> Key {
        entities.generate()
    }
}

macro_rules! impl_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: 'static + Send + Sync),+> AddEntity for ($(&mut SparseArray<$type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key {
                let key = entities.generate();

                $(
                    self.$index.insert(key.index(), component.$index);
                )+

                key
            }
        }
        impl<'a, $($type: 'static + Send + Sync),+> AddEntity for ($(Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(mut self, component: Self::Component, entities: &mut Entities) -> Key {
                <($(&mut SparseArray<$type>,)+)>::add_entity(($(&mut *self.$index,)+), component, entities)
            }
        }
        impl<'a, $($type: 'static + Send + Sync),+> AddEntity for ($(&mut Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key {
                <($(&mut SparseArray<$type>,)+)>::add_entity(($(&mut *self.$index,)+), component, entities)
            }
        }
    }
}

macro_rules! add_entity {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_add_entity![$(($left_type, $left_index))*];
        add_entity![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_add_entity![$(($type, $index))*];
    }
}

add_entity![(A, 0);  (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
