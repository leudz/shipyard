use crate::entity::{EntitiesView, Key};
use crate::error;
use crate::sparse_set::{Pack, ViewMut};
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
            match self.pack_info.pack {
                Pack::Tight(_) => Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>())),
                Pack::Loose(_) => Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>())),
                Pack::Update(_) => {
                    if self.pack_info.observer_types.is_empty() {
                        self.insert(component, entity);
                        Ok(())
                    } else {
                        Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>()))
                    }
                }
                Pack::NoPack => {
                    if self.pack_info.observer_types.is_empty() {
                        self.insert(component, entity);
                        Ok(())
                    } else {
                        Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>()))
                    }
                }
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
                    // checks if the caller has passed all necessary storages
                    // and list components we can pack
                    let mut should_pack = Vec::new();
                    // non packed storages should not pay the price of pack
                    if $(std::mem::discriminant(&self.$index.pack_info.pack) != std::mem::discriminant(&Pack::NoPack) || !self.$index.pack_info.observer_types.is_empty())||+ {
                        let mut type_ids = [$(TypeId::of::<$type>()),+];
                        type_ids.sort_unstable();
                        let mut add_types = [$(TypeId::of::<$add_type>()),*];
                        add_types.sort_unstable();
                        let mut real_types = Vec::with_capacity(type_ids.len() + add_types.len());
                        real_types.extend_from_slice(&type_ids);

                        $(
                            if self.$add_index.contains(entity) {
                                real_types.push(TypeId::of::<$add_type>());
                            }
                        )*
                        real_types.sort_unstable();

                        should_pack.reserve(real_types.len());
                        $(
                            if self.$index.pack_info.check_types(&type_ids, &add_types).is_err() {
                                return Err(error::AddComponent::MissingPackStorage(TypeId::of::<$type>()));
                            } else {
                                if !should_pack.contains(&TypeId::of::<$type>()) {
                                    match &self.$index.pack_info.pack {
                                        Pack::Tight(pack) => if let Ok(types) = pack.check_types(&real_types) {
                                            should_pack.extend_from_slice(types);
                                        }
                                        Pack::Loose(pack) => if let Ok(types) = pack.check_all_types(&real_types) {
                                            should_pack.extend_from_slice(types);
                                        }
                                        Pack::Update(_) => {}
                                        Pack::NoPack => {}
                                    }
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
