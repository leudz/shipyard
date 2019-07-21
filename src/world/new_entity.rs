use crate::component_storage::AllStorages;
use crate::component_storage::ComponentStorage;
use crate::entity::{Entities, Key};
use std::any::TypeId;

// Store components in a new entity
// If no storage exists for a component it will be created
pub trait WorldNewEntity {
    fn new(self, all_storages: &mut AllStorages, entities: &mut Entities) -> Key;
}

macro_rules! impl_new_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static + Send + Sync),+> WorldNewEntity for ($($type,)+) {
            fn new(self, all_storages: &mut AllStorages, entities: &mut Entities) -> Key {
                let key = entities.generate();

                $({
                    let type_id = TypeId::of::<$type>();
                    let mut array = all_storages.0.entry(type_id).or_insert_with(|| {
                        ComponentStorage::new::<$type>()
                    }).array_mut().unwrap();
                    array.insert(key.index(), self.$index);
                })+

                key
            }
        }
    }
}

macro_rules! new_entity {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_new_entity![$(($left_type, $left_index))*];
        new_entity![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_new_entity![$(($type, $index))*];
    }
}

new_entity![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
