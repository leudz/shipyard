use crate::storage::EntityId;
use crate::view::ViewMut;

/// Trait used to delete component(s).
pub trait Delete {
    /// Deletes the component(s) of an entity, they won't be returned.  
    /// A tuple is always needed, even for a single view.  
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Delete, EntitiesViewMut, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         let entity = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    ///
    ///         (&mut usizes, &mut u32s).delete(entity);
    ///     },
    /// );
    /// ```
    fn delete(self, entity: EntityId);
}

macro_rules! impl_delete_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: 'static),+> Delete for ($(&'_ mut ViewMut<'_, $type>,)+) {
            fn delete(self, entity: EntityId) {
                $(
                    self.$index.delete(entity);
                )+
            }
        }
    }
}

macro_rules! delete_component {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete_component![$(($type, $index))*];
        delete_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_delete_component![$(($type, $index))*];
    }
}

delete_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
