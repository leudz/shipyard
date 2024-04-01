use crate::component::Component;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::view::ViewMut;

/// Deletes component from entities.
pub trait Delete {
    /// Deletes component in `entity`, return `true` if the entity had this component.  
    /// Multiple components can be deleted at the same time using a tuple.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, Delete, ViewMut, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U64(u64);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((USIZE(0), U64(1)));
    ///
    /// let (mut usizes, mut u64s) = world.borrow::<(ViewMut<USIZE>, ViewMut<U64>)>().unwrap();
    ///
    /// assert!((&mut usizes, &mut u64s).delete(entity));
    /// ```
    fn delete(&mut self, entity: EntityId) -> bool;
}

impl Delete for () {
    #[inline]
    fn delete(&mut self, _: EntityId) -> bool {
        false
    }
}

impl<T: Component> Delete for ViewMut<'_, T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        let current = self.current;
        SparseSet::delete(&mut *self, entity, current)
    }
}

impl<T: Component> Delete for &mut ViewMut<'_, T> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        let current = self.current;
        SparseSet::delete(*self, entity, current)
    }
}

macro_rules! impl_delete_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: Delete),+> Delete for ($($storage,)+) {
            #[inline]
            fn delete(&mut self, entity: EntityId) -> bool {
                $(
                    self.$index.delete(entity)
                )||+
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
