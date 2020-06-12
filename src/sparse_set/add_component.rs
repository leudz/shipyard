use crate::error;
use crate::sparse_set::Pack;
use crate::storage::{EntityId, StorageId};
use crate::view::ViewMut;
use alloc::vec::Vec;
use core::any::{type_name, TypeId};

/// Adds components to an existing entity.
pub trait AddComponentUnchecked<T> {
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::try_add_component`] if you're unsure.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{World, EntitiesViewMut, ViewMut, AddComponentUnchecked};
    ///
    /// let world = World::new();
    ///
    /// let entity = world.borrow::<EntitiesViewMut>().add_entity((), ());
    ///
    /// world.run(|mut u32s: ViewMut<u32>| {
    ///     u32s.try_add_component_unchecked(0, entity).unwrap();
    /// });
    /// ```
    ///
    /// [`Entities::try_add_component`]: https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.try_add_component
    fn try_add_component_unchecked(
        self,
        component: T,
        entity: EntityId,
    ) -> Result<(), error::AddComponent>;
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::add_component`] if you're unsure.  
    /// Unwraps errors.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{World, EntitiesViewMut, ViewMut, AddComponentUnchecked};
    ///
    /// let world = World::new();
    ///
    /// let entity = world.borrow::<EntitiesViewMut>().add_entity((), ());
    ///
    /// world.run(|mut u32s: ViewMut<u32>| {
    ///     u32s.add_component_unchecked(0, entity);
    /// });
    /// ```
    ///
    /// [`Entities::add_component`]: https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_component
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    fn add_component_unchecked(self, component: T, entity: EntityId);
}

impl<T: 'static> AddComponentUnchecked<T> for &mut ViewMut<'_, T> {
    fn try_add_component_unchecked(
        self,
        component: T,
        entity: EntityId,
    ) -> Result<(), error::AddComponent> {
        match self.pack_info.pack {
            Pack::Tight(_) => Err(error::AddComponent::MissingPackStorage(type_name::<T>())),
            Pack::Loose(_) => Err(error::AddComponent::MissingPackStorage(type_name::<T>())),
            Pack::Update(_) => {
                if self.pack_info.observer_types.is_empty() {
                    self.insert(component, entity);
                    Ok(())
                } else {
                    Err(error::AddComponent::MissingPackStorage(type_name::<T>()))
                }
            }
            Pack::NoPack => {
                if self.pack_info.observer_types.is_empty() {
                    self.insert(component, entity);
                    Ok(())
                } else {
                    Err(error::AddComponent::MissingPackStorage(type_name::<T>()))
                }
            }
        }
    }
    #[cfg(feature = "panic")]
    fn add_component_unchecked(self, component: T, entity: EntityId) {
        self.try_add_component_unchecked(component, entity).unwrap();
    }
}

macro_rules! impl_add_component_unchecked {
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponentUnchecked<($($type,)+)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_add_component_unchecked(self, component: ($($type,)+), entity: EntityId) -> Result<(), error::AddComponent> {
                    // checks if the caller has passed all necessary storages
                    // and list components we can pack
                    let mut should_pack = Vec::new();
                    // non packed storages should not pay the price of pack
                    if $(core::mem::discriminant(&self.$index.pack_info.pack) != core::mem::discriminant(&Pack::NoPack) || !self.$index.pack_info.observer_types.is_empty())||+ {
                        let mut storage_ids = [$(StorageId::TypeId(TypeId::of::<$type>().into())),+];
                        storage_ids.sort_unstable();
                        let mut add_types = [$(TypeId::of::<$add_type>().into()),*];
                        add_types.sort_unstable();
                        let mut real_types = Vec::with_capacity(storage_ids.len() + add_types.len());
                        real_types.extend_from_slice(&storage_ids);

                        $(
                            if self.$add_index.contains(entity) {
                                real_types.push(TypeId::of::<$add_type>().into());
                            }
                        )*
                        real_types.sort_unstable();

                        should_pack.reserve(real_types.len());
                        $(
                            if self.$index.pack_info.has_all_storages(&storage_ids, &add_types) {
                                if !should_pack.contains(&TypeId::of::<$type>().into()) {
                                    match &self.$index.pack_info.pack {
                                        Pack::Tight(pack) => if let Ok(types) = pack.is_packable(&real_types) {
                                            should_pack.extend_from_slice(types);
                                        }
                                        Pack::Loose(pack) => if let Ok(types) = pack.is_packable(&real_types) {
                                            should_pack.extend_from_slice(types);
                                        }
                                        Pack::Update(_) => {}
                                        Pack::NoPack => {}
                                    }
                                }
                            } else {
                                return Err(error::AddComponent::MissingPackStorage(type_name::<$type>()));
                            }
                        )+

                        $(
                            if should_pack.contains(&TypeId::of::<$add_type>().into()) {
                                self.$add_index.pack(entity);
                            }
                        )*
                    }

                    $(
                        self.$index.insert(component.$index, entity);
                        if should_pack.contains(&TypeId::of::<$type>().into()) {
                            self.$index.pack(entity);
                        }
                    )+

                    Ok(())
            }
            #[cfg(feature = "panic")]
            fn add_component_unchecked(self, component: ($($type,)+), entity: EntityId) {
                self.try_add_component_unchecked(component, entity).unwrap();
            }
        }
    }
}

macro_rules! add_component_unchecked {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component_unchecked![($type1, $index1) $(($type, $index))*;];
        add_component_unchecked![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component_unchecked![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        add_component_unchecked![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_add_component_unchecked![$(($type, $index))+;];
    }
}

add_component_unchecked![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
