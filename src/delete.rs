use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::view::ViewMut;

/// Trait used to delete component(s).
pub trait Delete {
    /// Deletes the component(s) of an entity, they won't be returned.  
    /// Returns `true` if all storages deleted a component.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Delete, ViewMut, World};
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((0usize, 1u32));
    ///
    /// let (mut usizes, mut u32s) = world.borrow::<(ViewMut<usize>, ViewMut<u32>)>().unwrap();
    /// (&mut usizes, &mut u32s).delete(entity);
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
