use crate::error;
use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::views::ViewMut;
use alloc::vec::Vec;
use core::any::TypeId;

pub trait Delete<T> {
    fn try_delete(self, entity: EntityId) -> Result<(), error::Remove>;
    fn delete(self, entity: EntityId);
}

macro_rules! impl_delete {
    // add is short for additional
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> Delete<($($type,)*)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_delete(self, entity: EntityId) -> Result<(), error::Remove> {
                // non packed storages should not pay the price of pack
                if $(core::mem::discriminant(&self.$index.pack_info.pack) != core::mem::discriminant(&Pack::NoPack) || !self.$index.pack_info.observer_types.is_empty())||+ {
                    let mut types = [$(TypeId::of::<$type>()),+];
                    types.sort_unstable();
                    let mut add_types = [$(TypeId::of::<$add_type>()),*];
                    add_types.sort_unstable();

                    let mut should_unpack = Vec::with_capacity(types.len() + add_types.len());
                    $(
                        match self.$index.pack_info.check_types(&types, &add_types) {
                            Ok(_) => match &self.$index.pack_info.pack {
                                Pack::Tight(pack) => {
                                    should_unpack.extend_from_slice(&pack.types);
                                    should_unpack.extend_from_slice(&self.$index.pack_info.observer_types);
                                }
                                Pack::Loose(pack) => {
                                    should_unpack.extend_from_slice(&pack.tight_types);
                                    should_unpack.extend_from_slice(&self.$index.pack_info.observer_types);
                                }
                                Pack::Update(_) => should_unpack.extend_from_slice(&self.$index.pack_info.observer_types),
                                Pack::NoPack => should_unpack.extend_from_slice(&self.$index.pack_info.observer_types),
                            }
                            Err(_) => return Err(error::Remove::MissingPackStorage(TypeId::of::<$type>()))
                        }
                    )+

                    $(
                        if should_unpack.contains(&TypeId::of::<$add_type>()) {
                            self.$add_index.unpack(entity);
                        }
                    )*
                }

                $(
                    self.$index.actual_delete(entity);
                )+

                Ok(())
            }
            fn delete(self, entity: EntityId) {
                Delete::<($($type,)+)>::try_delete(self, entity).unwrap()
            }
        }
    }
}

macro_rules! delete {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete![($type1, $index1) $(($type, $index))*;];
        delete![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        delete![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_delete![$(($type, $index))+;];
    }
}

delete![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
