use crate::entity::Key;
use crate::error;
use crate::sparse_array::{SparseArray, ViewMut, Write};
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
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let entity1 = world.new_entity((0usize, 1u32));
    ///
    /// world.run::<(&mut usize, &mut u32), _>(|(mut usizes, mut u32s)| {
    ///     let old = Remove::<(usize, u32)>::try_remove((&mut usizes, &mut u32s), entity1).unwrap();
    ///     assert_eq!(old, (Some(0), Some(1)));
    /// });
    /// ```
    /// When using packed storages you have to pass all storages packed with it,
    /// even if you don't remove any component from it.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.pack_owned::<(usize, u32)>();
    /// let entity1 = world.new_entity((0usize, 1u32));
    ///
    /// world.run::<(&mut usize, &mut u32), _>(|(mut usizes, mut u32s)| {
    ///     let old = Remove::<(usize,)>::try_remove((&mut usizes, &mut u32s), entity1).unwrap();
    ///     assert_eq!(old, (Some(0),));
    /// });
    /// ```
    fn try_remove(self, entity: Key) -> Result<T::Out, error::Remove>;
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
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let entity1 = world.new_entity((0usize, 1u32));
    ///
    /// world.run::<(&mut usize, &mut u32), _>(|(mut usizes, mut u32s)| {
    ///     let old = Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(0), Some(1)));
    /// });
    /// ```
    /// When using packed storages you have to pass all storages packed with it,
    /// even if you don't remove any component from it.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// world.pack_owned::<(usize, u32)>();
    /// let entity1 = world.new_entity((0usize, 1u32));
    ///
    /// world.run::<(&mut usize, &mut u32), _>(|(mut usizes, mut u32s)| {
    ///     let old = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(0),));
    /// });
    /// ```
    fn remove(self, entity: Key) -> T::Out;
}

impl<T: 'static> Remove<(T,)> for &mut SparseArray<T> {
    fn try_remove(self, entity: Key) -> Result<<(T,) as Removable>::Out, error::Remove> {
        if !self.is_packed_owned() {
            Ok((self.remove(entity.index()),))
        } else {
            Err(error::Remove::MissingPackStorage(TypeId::of::<T>()))
        }
    }
    fn remove(self, entity: Key) -> <(T,) as Removable>::Out {
        self.try_remove(entity).unwrap()
    }
}

impl<T: 'static> Remove<(T,)> for &mut ViewMut<'_, T> {
    fn try_remove(self, entity: Key) -> Result<<(T,) as Removable>::Out, error::Remove> {
        if !self.is_packed_owned() {
            Ok((self.remove(entity.index()),))
        } else {
            Err(error::Remove::MissingPackStorage(TypeId::of::<T>()))
        }
    }
    fn remove(self, entity: Key) -> <(T,) as Removable>::Out {
        self.try_remove(entity).unwrap()
    }
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
        impl<$($type: 'static,)+ $($add_type: 'static),*> Remove<($($type,)+)> for ($(&mut SparseArray<$type>,)+ $(&mut SparseArray<$add_type>,)*) {
            fn try_remove(self, entity: Key) -> Result<<($($type,)+) as Removable>::Out, error::Remove> {
                Remove::<($($type,)+)>::try_remove(($(&mut self.$index.view_mut(),)+ $(&mut self.$add_index.view_mut(),)*), entity)
            }
            fn remove(self, entity: Key) -> <($($type,)+) as Removable>::Out {
                Remove::<($($type,)+)>::try_remove(self, entity).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> Remove<($($type,)*)> for ($(Write<'_, $type>,)+ $(Write<'_, $add_type>,)*) {
            fn try_remove(mut self, entity: Key) -> Result<<($($type,)+) as Removable>::Out, error::Remove> {
                Remove::<($($type,)+)>::try_remove(($(&mut *self.$index,)+ $(&mut *self.$add_index,)*), entity)
            }
            fn remove(self, entity: Key) -> <($($type,)+) as Removable>::Out {
                Remove::<($($type,)+)>::try_remove(self, entity).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> Remove<($($type,)*)> for ($(&mut Write<'_, $type>,)+ $(&mut Write<'_, $add_type>,)*) {
            fn try_remove(self, entity: Key) -> Result<<($($type,)+) as Removable>::Out, error::Remove> {
                Remove::<($($type,)+)>::try_remove(($(&mut **self.$index,)+ $(&mut **self.$add_index,)*), entity)
            }
            fn remove(self, entity: Key) -> <($($type,)+) as Removable>::Out {
                Remove::<($($type,)+)>::try_remove(self, entity).unwrap()
            }
        }

        impl<$($type: 'static,)+ $($add_type: 'static),*> Remove<($($type,)*)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_remove(self, entity: Key) -> Result<<($($type,)+) as Removable>::Out, error::Remove> {
                if $(self.$index.is_packed_owned())||+ {
                    let type_ids = [$(TypeId::of::<$type>(),)+ $(TypeId::of::<$add_type>()),*];
                    let mut sorted_type_ids = type_ids.clone();
                    sorted_type_ids.sort_unstable();
                    let i = type_ids.len();

                    let mut pack_types = Vec::with_capacity(i);
                    $({
                        let type_id = TypeId::of::<$type>();
                        if !pack_types.contains(&type_id) {
                            let new_pack_types = self.$index.should_pack_owned(&sorted_type_ids);
                            if self.$index.is_packed_owned() && new_pack_types.len() == 0 {
                                return Err(error::Remove::MissingPackStorage(type_id));
                            } else {
                                pack_types.extend(new_pack_types.iter().filter(|&&x| x == type_id));
                            }
                        }
                    })+

                    Ok(($(
                        self.$index.remove(entity.index()),
                    )+))
                } else {
                    Ok(($(
                        self.$index.remove(entity.index()),
                    )+))
                }
            }
            fn remove(self, entity: Key) -> <($($type,)+) as Removable>::Out {
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

remove![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
