use crate::component::Component;
use crate::entity_id::EntityId;
use crate::ViewMut;

/// Add component only if not already present.
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
    /// struct U64(u64);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut u64s) = world.borrow::<(EntitiesViewMut, ViewMut<U64>)>().unwrap();
    /// let entity = entities.add_entity((), ());
    ///
    /// assert!(u64s.add_distinct_component_unchecked(entity, U64(0)));
    /// assert!(!u64s.add_distinct_component_unchecked(entity, U64(0)));
    /// ```
    ///
    /// [`Entities::add_component`]: crate::Entities::add_component()
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

        self.sparse_set.insert(entity, component, self.current);
        true
    }
}

impl<T: Component + PartialEq> AddDistinctComponent for &mut ViewMut<'_, T> {
    type Component = T;

    #[inline]
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

        self.sparse_set.insert(entity, component, self.current);
        true
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

add_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
