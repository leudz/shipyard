use crate::entity::{EntitiesView, Key};
use crate::error;
use crate::sparse_array::{PackInfo, ViewMut};
use std::any::TypeId;

// No new storage will be created
/// Adds components to an existing entity without creating new storage.
pub trait AddComponent<T> {
    fn try_add_component(
        self,
        component: T,
        entity: Key,
        entities: &EntitiesView,
    ) -> Result<(), error::AddComponent>;
    fn add_component(self, component: T, entity: Key, entities: &EntitiesView);
}

impl<T: 'static> AddComponent<T> for &mut ViewMut<'_, T> {
    fn try_add_component(
        self,
        component: T,
        entity: Key,
        entities: &EntitiesView,
    ) -> Result<(), error::AddComponent> {
        if entities.is_alive(entity) {
            match self.pack_info {
                PackInfo::NoPack => {
                    self.insert(component, entity);
                    Ok(())
                }
                _ => Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>())),
            }
        } else {
            Err(error::AddComponent::EntityIsNotAlive)
        }
    }
    fn add_component(self, component: T, entity: Key, entities: &EntitiesView) {
        self.try_add_component(component, entity, entities).unwrap()
    }
}

macro_rules! impl_add_component {
    // add is short for additional
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)+)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) -> Result<(), error::AddComponent> {
                if entities.is_alive(entity) {

                    // non packed storages should not pay the price of pack
                    let mut packed = false;
                    $(
                        if std::mem::discriminant(self.$index.pack_info) != std::mem::discriminant(&PackInfo::NoPack) {
                            packed = true;
                        }
                    )+

                    // checks if the caller has passed all necessary storages
                    // and list components we can pack
                    let mut should_pack = Vec::new();
                    if packed {
                        let mut all_types = [$(TypeId::of::<$type>(),)+ $(TypeId::of::<$add_type>()),*];
                        all_types.sort_unstable();
                        let mut type_ids = vec![$(TypeId::of::<$type>()),+];

                        $(
                            if self.$add_index.contains(entity) {
                                type_ids.push(TypeId::of::<$add_type>());
                            }
                        )*

                        type_ids.sort_unstable();

                        should_pack.reserve(type_ids.len());
                        $(
                            if !should_pack.contains(&TypeId::of::<$type>()) {
                                match self.$index.pack_info {
                                    PackInfo::Tight(pack) => {
                                        let (missing_storage, is_packed) = pack.check_types(&type_ids, &all_types);
                                        if missing_storage {
                                            return Err(error::AddComponent::MissingPackStorage(TypeId::of::<$type>()))
                                        }
                                        if is_packed {
                                            should_pack.extend_from_slice(&pack.types);
                                        }
                                    },
                                    PackInfo::Loose(pack) => {
                                        let (missing_storage, is_packed) = pack.check_types(&type_ids, &all_types);
                                        if missing_storage {
                                            return Err(error::AddComponent::MissingPackStorage(TypeId::of::<$type>()))
                                        }
                                        if is_packed {
                                            should_pack.extend_from_slice(&pack.tight_types);
                                        }
                                    },
                                    PackInfo::NoPack => {}
                                }
                            }
                        )+

                        $(
                            if should_pack.contains(&TypeId::of::<$add_type>()) {
                                self.$add_index.pack(entity);
                            }
                        )*
                    }

                    $(
                        self.$index.insert(component.$index, entity);
                        if should_pack.contains(&TypeId::of::<$type>()) {
                            self.$index.pack(entity);
                        }
                    )+

                    Ok(())
                } else {
                    Err(error::AddComponent::EntityIsNotAlive)
                }
            }
            fn add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) {
                self.try_add_component(component, entity, entities).unwrap()
            }
        }
    }
}

macro_rules! add_component {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![($type1, $index1) $(($type, $index))*;];
        add_component![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        add_component![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_add_component![$(($type, $index))+;];
    }
}

add_component![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
