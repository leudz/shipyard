use crate::sparse_set::SparseSet;
use crate::storage::EntityId;
use crate::view::ViewMut;

/// Trait used to delete component(s).
pub trait Delete {
    /// Deletes the component(s) of an entity, they won't be returned.  
    /// Returns `true` if all storages deleted a component.
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
    fn delete(&mut self, entity: EntityId) -> bool;
}

impl Delete for () {
    #[inline]
    fn delete(&mut self, _: EntityId) -> bool {
        false
    }
}

impl<T: 'static> Delete for ViewMut<'_, T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        SparseSet::delete(&mut *self, entity)
    }
}

impl<T: 'static> Delete for &mut ViewMut<'_, T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        SparseSet::delete(&mut *self, entity)
    }
}

macro_rules! impl_delete_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: Delete),+> Delete for ($($storage,)+) {
            #[inline]
            fn delete(&mut self, entity: EntityId) -> bool {
                $(
                    self.$index.delete(entity)
                )&&+
            }
        }
    }
}

macro_rules! delete_component {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_delete_component![$(($storage, $index))*];
        delete_component![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_delete_component![$(($storage, $index))*];
    }
}

delete_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
