#[cfg(test)]
mod tests;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::ViewMut;

/// Adds components to an existing entity only if not already present.
pub trait AddDistinctComponent {
    #[allow(missing_docs)]
    type Component;
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// If the entity already has this component, it won't be replaced. Very useful if you want accurate modification tracking.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::add_component`] if you're unsure.
    ///
    /// Returns `true` if the component was added.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{AddDistinctComponent, Component, EntitiesViewMut, ViewMut, World};
    ///
    /// #[derive(Component, PartialEq)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();
    /// let entity = entities.add_entity((), ());
    ///
    /// assert!(u32s.add_distinct_component_unchecked(entity, U32(0)));
    /// assert!(!u32s.add_distinct_component_unchecked(entity, U32(0)));
    /// ```
    ///
    /// [`Entities::add_component`]: crate::entities::Entities::add_component()
    fn add_distinct_component_unchecked(
        &mut self,
        entity: EntityId,
        component: Self::Component,
    ) -> bool;
}

impl AddDistinctComponent for () {
    type Component = ();

    #[inline]
    fn add_distinct_component_unchecked(&mut self, _: EntityId, _: Self::Component) -> bool {
        false
    }
}

impl<T: Component + PartialEq> AddDistinctComponent for ViewMut<'_, T> {
    type Component = T;

    #[inline]
    #[track_caller]
    fn add_distinct_component_unchecked(
        &mut self,
        entity: EntityId,
        component: Self::Component,
    ) -> bool {
        if let Some(c) = self.sparse_set.private_get(entity) {
            if *c == component {
                return false;
            }
        }

        self.sparse_set
            .insert(entity, component, self.current)
            .was_inserted()
    }
}

impl<T: Component + PartialEq> AddDistinctComponent for &mut ViewMut<'_, T> {
    type Component = T;

    #[inline]
    #[track_caller]
    fn add_distinct_component_unchecked(
        &mut self,
        entity: EntityId,
        component: Self::Component,
    ) -> bool {
        if let Some(c) = self.sparse_set.private_get(entity) {
            if *c == component {
                return false;
            }
        }

        self.sparse_set
            .insert(entity, component, self.current)
            .was_inserted()
    }
}

macro_rules! impl_add_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: AddDistinctComponent,)+> AddDistinctComponent for ($($storage,)+) {
            type Component = ($($storage::Component,)+);

            #[inline]
            fn add_distinct_component_unchecked(&mut self, entity: EntityId, component: Self::Component) -> bool {
                $(
                    self.$index.add_distinct_component_unchecked(entity, component.$index)
                )||+
            }
        }
    }
}

macro_rules! add_component {
    ($(($storage: ident, $index: tt))+; ($storage1: ident, $index1: tt) $(($queue_storage: ident, $queue_index: tt))*) => {
        impl_add_component![$(($storage, $index))*];
        add_component![$(($storage, $index))* ($storage1, $index1); $(($queue_storage, $queue_index))*];
    };
    ($(($storage: ident, $index: tt))+;) => {
        impl_add_component![$(($storage, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
add_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
add_component![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
