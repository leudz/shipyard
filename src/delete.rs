use crate::component::Component;
use crate::entity_id::EntityId;
use crate::views::ViewMut;

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
    /// assert!((&mut usizes, &mut u32s).delete(entity));
    /// ```
    fn delete(&mut self, entity: EntityId) -> bool;
}

impl Delete for () {
    #[inline]
    fn delete(&mut self, _: EntityId) -> bool {
        false
    }
}

impl<T: Component, TRACK> Delete for ViewMut<'_, T, TRACK> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        let current = self.current;
        self.dyn_delete(entity, current)
    }
}

impl<T: Component, TRACK> Delete for &mut ViewMut<'_, T, TRACK> {
    #[inline]
    fn delete(&mut self, entity: EntityId) -> bool {
        let current = self.current;
        self.dyn_delete(entity, current)
    }
}

macro_rules! impl_delete_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: Delete),+> Delete for ($($storage,)+) {
            #[inline]
            fn delete(&mut self, entity: EntityId) -> bool {
                $(
                    self.$index.delete(entity)
                )|+
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

#[cfg(not(feature = "extended_tuple"))]
delete_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
delete_component![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
