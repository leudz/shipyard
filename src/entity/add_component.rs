use crate::entity::{EntitiesView, Key};
use crate::error;
use crate::sparse_array::{SparseArray, ViewMut, Write};
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

impl<T: 'static> AddComponent<T> for &mut SparseArray<T> {
    fn try_add_component(
        self,
        component: T,
        entity: Key,
        entities: &EntitiesView,
    ) -> Result<(), error::AddComponent> {
        (&mut self.view_mut()).try_add_component(component, entity, entities)
    }
    fn add_component(self, component: T, entity: Key, entities: &EntitiesView) {
        self.try_add_component(component, entity, entities).unwrap()
    }
}

impl<T: 'static> AddComponent<T> for &mut ViewMut<'_, T> {
    fn try_add_component(
        self,
        component: T,
        entity: Key,
        entities: &EntitiesView,
    ) -> Result<(), error::AddComponent> {
        if entities.is_alive(entity) {
            if !self.is_packed_owned() {
                self.insert(component, entity);
                Ok(())
            } else {
                Err(error::AddComponent::MissingPackStorage(TypeId::of::<T>()))
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
        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)*)> for ($(&mut SparseArray<$type>,)+ $(&mut SparseArray<$add_type>,)*) {
            fn try_add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) -> Result<(), error::AddComponent> {
                ($(&mut self.$index.view_mut(),)+ $(&mut self.$add_index.view_mut(),)*).try_add_component(component, entity, entities)
            }
            fn add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) {
                self.try_add_component(component, entity, entities).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)*)> for ($(Write<'_, $type>,)+ $(Write<'_, $add_type>,)*) {
            fn try_add_component(mut self, component: ($($type,)+), entity: Key, entities: &EntitiesView) -> Result<(), error::AddComponent> {
                ($(&mut *self.$index,)+ $(&mut *self.$add_index,)*).try_add_component(component, entity, entities)
            }
            fn add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) {
                self.try_add_component(component, entity, entities).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)*)> for ($(&mut Write<'_, $type>,)+ $(&mut Write<'_, $add_type>,)*) {
            fn try_add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) -> Result<(), error::AddComponent> {
                ($(&mut **self.$index,)+ $(&mut **self.$add_index,)*).try_add_component(component, entity, entities)
            }
            fn add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) {
                self.try_add_component(component, entity, entities).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)+)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_add_component(self, component: ($($type,)+), entity: Key, entities: &EntitiesView) -> Result<(), error::AddComponent> {
                if entities.is_alive(entity) {
                    if $(self.$index.is_packed_owned())||+ {
                        let mut type_ids = vec![$(TypeId::of::<$type>(),)+];
                        type_ids.sort_unstable();
                        // checks if the caller has passed all necessary storages
                        let mut storage_type_ids = type_ids.clone();
                        storage_type_ids.extend_from_slice(&[$(TypeId::of::<$add_type>()),*]);
                        storage_type_ids.sort_unstable();
                        $(
                            if self.$index.is_packed_owned() && self.$index.should_pack_owned(&storage_type_ids).is_empty() {
                                return Err(error::AddComponent::MissingPackStorage(TypeId::of::<$type>()));
                            }
                        )+
                        // add the component to the storage
                        $(
                            self.$index.insert(component.$index, entity);
                        )+

                        // add additional types if the entity has this component
                        $(
                            if self.$add_index.contains(entity) {
                                type_ids.push(TypeId::of::<$add_type>());
                            }
                        )*
                        type_ids.sort_unstable();

                        // keeps track of types to pack
                        let mut should_pack = Vec::with_capacity(type_ids.len());
                        $(
                            let type_id = TypeId::of::<$type>();

                            if should_pack.contains(&type_id) {
                                self.$index.pack(entity);
                            } else {
                                let pack_types = self.$index.should_pack_owned(&type_ids);

                                should_pack.extend(pack_types.iter().filter(|&&x| x != type_id));
                                if !pack_types.is_empty() {
                                    self.$index.pack(entity);
                                }
                            }
                        )+

                        $(
                            if should_pack.contains(&TypeId::of::<$add_type>()) {
                                self.$add_index.pack(entity);
                            }
                        )*
                    } else {
                        $(
                            self.$index.insert(component.$index, entity);
                        )+
                    }

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
