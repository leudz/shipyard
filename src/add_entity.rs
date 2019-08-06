use crate::entity::{Entities, Key};
use crate::sparse_array::{SparseArray, Write};
use std::any::TypeId;

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

impl<T: 'static + Send + Sync> AddEntity for (&mut SparseArray<T>,) {
    type Component = (T,);
    fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key {
        let key = entities.generate();

        self.0.insert(key.index(), component.0);

        key
    }
}

impl<'a, T: 'static + Send + Sync> AddEntity for (Write<'_, T>,) {
    type Component = (T,);
    fn add_entity(mut self, component: Self::Component, entities: &mut Entities) -> Key {
        (&mut *self.0,).add_entity(component, entities)
    }
}

impl<'a, T: 'static + Send + Sync> AddEntity for (&mut Write<'_, T>,) {
    type Component = (T,);
    fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key {
        (&mut **self.0,).add_entity(component, entities)
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

                let mut type_ids = [$(TypeId::of::<$type>()),+];
                type_ids.sort_unstable();

                let mut should_pack = Vec::with_capacity(type_ids.len());
                $(
                    let type_id = TypeId::of::<$type>();

                    if should_pack.contains(&type_id) {
                        self.$index.pack(key.index());
                    } else {
                        let pack_types = self.$index.should_pack_owned(&type_ids);

                        should_pack.extend(pack_types.iter().filter(|&&x| x == type_id));
                        if !pack_types.is_empty() {
                            self.$index.pack(key.index());
                        }
                    }
                )+

                key
            }
        }
        impl<'a, $($type: 'static + Send + Sync),+> AddEntity for ($(Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(mut self, component: Self::Component, entities: &mut Entities) -> Key {
                ($(&mut *self.$index,)+).add_entity(component, entities)
            }
        }
        impl<'a, $($type: 'static + Send + Sync),+> AddEntity for ($(&mut Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entities: &mut Entities) -> Key {
                ($(&mut **self.$index,)+).add_entity(component, entities)
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

add_entity![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
