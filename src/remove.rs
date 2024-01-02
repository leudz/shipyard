use crate::component::Component;
use crate::entity_id::EntityId;
use crate::views::ViewMut;

/// Removes component from entities.
pub trait Remove {
    /// Type of the removed component.
    type Out;
    /// Removes component in `entity`, if the entity had a component, they will be returned.  
    /// Multiple components can be removed at the same time using a tuple.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, Remove, ViewMut, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((USIZE(0), U32(1)));
    ///
    /// let (mut usizes, mut u32s) = world.borrow::<(ViewMut<USIZE>, ViewMut<U32>)>().unwrap();
    ///
    /// let old = (&mut usizes, &mut u32s).remove(entity);
    /// assert_eq!(old, (Some(USIZE(0)), Some(U32(1))));
    /// ```
    fn remove(&mut self, entity: EntityId) -> Self::Out;
}

impl Remove for () {
    type Out = ();

    #[inline]
    fn remove(&mut self, _: EntityId) -> Self::Out {}
}

impl<T: Component, TRACK> Remove for ViewMut<'_, T, TRACK> {
    type Out = Option<T>;

    #[inline]
    fn remove(&mut self, entity: EntityId) -> Self::Out {
        let current = self.current;
        self.dyn_remove(entity, current)
    }
}

impl<T: Component, TRACK> Remove for &mut ViewMut<'_, T, TRACK> {
    type Out = Option<T>;

    #[inline]
    fn remove(&mut self, entity: EntityId) -> Self::Out {
        let current = self.current;
        self.dyn_remove(entity, current)
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
