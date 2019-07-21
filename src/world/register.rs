use crate::component_storage::ComponentStorage;
use crate::world::World;
use std::any::TypeId;

// Register multiple storages at once
pub trait Register {
    fn register(world: &World);
}

impl Register for () {
    fn register(_: &World) {}
}

macro_rules! impl_register {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static + Send + Sync),+> Register for ($($type,)+) {
            fn register(world: &World) {
                let mut all_storages = world.storages.try_borrow_mut().unwrap();
                $({
                    let type_id = TypeId::of::<$type>();
                    all_storages.0.entry(type_id).or_insert_with(|| {
                        ComponentStorage::new::<$type>()
                    });
                })+
            }
        }
    }
}

macro_rules! register {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_register![$(($left_type, $left_index))*];
        register![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_register![$(($type, $index))*];
    }
}

register![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
