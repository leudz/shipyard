use crate::error;
use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::views::ViewMut;
use std::any::TypeId;

pub trait Removable {
    type Out;
}

/// Removes component from entities.
pub trait Remove<T: Removable> {
    /// Removes component in `entity`, if the entity had them, they will be returned.
    ///
    /// Multiple components can be removed at the same time using a tuple.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// The compiler has trouble inferring the return types.
    /// You'll often have to use the full path `Remove::<type>::try_remove`.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize, u32)>::try_remove((&mut usizes, &mut u32s), entity1).unwrap();
    ///     assert_eq!(old, (Some(0), Some(1)));
    /// });
    /// ```
    /// When using packed storages you have to pass all storages packed with it,
    /// even if you don't remove any component from it.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.tight_pack::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize,)>::try_remove((&mut usizes, &mut u32s), entity1).unwrap();
    ///     assert_eq!(old, (Some(0),));
    /// });
    /// ```
    fn try_remove(self, entity: EntityId) -> Result<T::Out, error::Remove>;
    /// Removes component in `entity`, if the entity had them, they will be returned.
    ///
    /// Multiple components can be removed at the same time using a tuple.
    ///
    /// `T` has to be a tuple even for a single type.
    /// In this case use (T,).
    ///
    /// The compiler has trouble inferring the return types.
    /// You'll often have to use the full path `Remove::<type>::remove`.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(0), Some(1)));
    /// });
    /// ```
    /// When using packed storages you have to pass all storages packed with it,
    /// even if you don't remove any component from it.
    /// # Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.tight_pack::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(0),));
    /// });
    /// ```
    fn remove(self, entity: EntityId) -> T::Out;
}

macro_rules! impl_removable {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type),+> Removable for ($($type,)+) {
            type Out = ($(Option<$type>,)+);
        }
    }
}

macro_rules! impl_remove {
    // add is short for additional
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> Remove<($($type,)*)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_remove(self, entity: EntityId) -> Result<<($($type,)+) as Removable>::Out, error::Remove> {
                // non packed storages should not pay the price of pack
                if $(std::mem::discriminant(&self.$index.pack_info.pack) != std::mem::discriminant(&Pack::NoPack) || !self.$index.pack_info.observer_types.is_empty())||+ {
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

                Ok(($(
                    self.$index.remove(entity),
                )+))
            }
            fn remove(self, entity: EntityId) -> <($($type,)+) as Removable>::Out {
                Remove::<($($type,)+)>::try_remove(self, entity).unwrap()
            }
        }
    }
}

macro_rules! remove {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_remove![($type1, $index1) $(($type, $index))*;];
        impl_removable![($type1, $index1) $(($type, $index))*];
        remove![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_remove![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        remove![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_remove![$(($type, $index))+;];
        impl_removable![$(($type, $index))+];
    }
}

remove![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
