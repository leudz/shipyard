use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::view::ViewMut;

/// Removes component from entities.
pub trait Remove {
    /// Type of the removed component.
    type Out;
    /// Removes component in `entity`, if the entity had a component, they will be returned.
    ///
    /// Multiple components can be removed at the same time using a tuple.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, Remove, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///     let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     let old = (&mut usizes, &mut u32s).remove(entity1);
    ///     assert_eq!(old, (Some(0), Some(1)));
    /// });
    /// ```
    fn remove(&mut self, entity: EntityId) -> Self::Out;
}

impl Remove for () {
    type Out = ();

    #[inline]
    fn remove(&mut self, _: EntityId) -> Self::Out {}
}

impl<T: 'static> Remove for ViewMut<'_, T> {
    type Out = Option<T>;

    #[inline]
    fn remove(&mut self, entity: EntityId) -> Self::Out {
        SparseSet::remove(&mut *self, entity)
    }
}

impl<T: 'static> Remove for &mut ViewMut<'_, T> {
    type Out = Option<T>;

    #[inline]
    fn remove(&mut self, entity: EntityId) -> Self::Out {
        SparseSet::remove(&mut *self, entity)
    }
}

macro_rules! impl_remove_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: Remove),+> Remove for ($($storage,)+) {
            type Out = ($($storage::Out,)+);

            #[inline]
            fn remove(&mut self, entity: EntityId) -> Self::Out {
                ($(
                    self.$index.remove(entity),
                )+)
            }
        }
    }
}

macro_rules! remove_component {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_remove_component![$(($storage, $index))*];
        remove_component![$(($storage, $index))* ($storage1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_remove_component![$(($storage, $index))*];
    }
}

remove_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
