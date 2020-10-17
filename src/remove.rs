use crate::sparse_set::{OldComponent};
use crate::storage::EntityId;
use crate::view::ViewMut;

/// Removes component from entities.
pub trait Remove {
    type Out;
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
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, OldComponent, Remove, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(OldComponent::Owned(0)), Some(OldComponent::Owned(1))));
    /// });
    /// ```
    /// When using packed storages you have to pass all storages packed with it,
    /// even if you don't remove any component from it.
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, OldComponent, Remove, TightPack, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///     (&mut usizes, &mut u32s).tight_pack();
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
    ///     assert_eq!(old, (Some(OldComponent::Owned(0)),));
    /// });
    /// ```
    fn remove(self, entity: EntityId) -> Self::Out;
}

macro_rules! impl_remove_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type),+> Remove for ($(&'_ mut ViewMut<'_, $type>,)+) {
            type Out = ($(Option<OldComponent<$type>>,)+);

            fn remove(self, entity: EntityId) -> Self::Out {
                ($(
                    self.$index.actual_remove(entity),
                )+)
            }
        }
    }
}

macro_rules! remove_component {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_remove_component![$(($type, $index))*];
        remove_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_remove_component![$(($type, $index))*];
    }
}

remove_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
