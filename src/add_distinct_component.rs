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

add_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{track, Component, EntitiesViewMut, Get, ViewMut, World};

    #[derive(PartialEq, Debug)]
    struct USIZE(usize);

    impl Component for USIZE {
        type Tracking = track::InsertionAndModification;
    }

    /// Make sure that:
    /// - `add_distinct_component_unchecked` inserts the component when no component is present.
    /// - it correctly reports the success.
    /// - the component is flagged as inserted.
    /// - the component is not flagged as modified.
    #[test]
    fn add_no_component() {
        let mut world = World::new();

        let eid = world.add_entity(());

        let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

        let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

        assert!(was_inserted);
        assert_eq!(usizes.get(eid).unwrap(), &USIZE(0));
        assert!(usizes.is_inserted(eid));
        assert!(!usizes.is_modified(eid));
    }

    /// Make sure that:
    /// - `add_distinct_component_unchecked` inserts the component when a distinct component is present.
    /// - it correctly reports the success.
    /// - the component is not flagged as inserted.
    /// - the component is flagged as modified.
    #[test]
    fn add_distinct_component() {
        let mut world = World::new();

        let eid = world.add_entity(USIZE(1));

        world.run(|usizes: ViewMut<'_, USIZE>| {
            usizes.clear_all_inserted();
        });

        let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

        let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

        assert!(was_inserted);
        assert_eq!(usizes.get(eid).unwrap(), &USIZE(0));
        assert!(!usizes.is_inserted(eid));
        assert!(usizes.is_modified(eid));
    }

    /// Make sure that:
    /// - `add_distinct_component_unchecked` does not insert the component when an equal component is present.
    /// - it correctly reports the failure.
    /// - the component is not flagged as inserted.
    /// - the component is not flagged as modified.
    #[test]
    fn add_identical_component() {
        let mut world = World::new();

        let eid = world.add_entity(USIZE(0));

        world.run(|usizes: ViewMut<'_, USIZE>| {
            usizes.clear_all_inserted();
        });

        let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

        let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

        assert!(!was_inserted);
        assert!(!usizes.is_inserted(eid));
        assert!(!usizes.is_modified(eid));
    }

    /// Make sure that:
    /// - `add_distinct_component_unchecked` does not insert the component when a component from an entity with a larger generation is present.
    /// - it correctly reports the failure.
    /// - the component is not flagged as inserted.
    /// - the component is not flagged as modified.
    #[test]
    fn add_smaller_gen_component() {
        let mut world = World::new();

        let eid1 = world.add_entity(());
        world.delete_entity(eid1);
        let eid2 = world.add_entity(USIZE(1));

        assert_eq!(eid1.index(), eid2.index());

        let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

        let was_inserted = usizes.add_distinct_component_unchecked(eid1, USIZE(0));

        assert!(usizes.get(eid1).is_err());
        assert_eq!(usizes.get(eid2).unwrap(), &USIZE(1));

        assert!(!was_inserted);
        assert!(!usizes.is_inserted(eid1));
        assert!(!usizes.is_modified(eid1));
    }

    /// Make sure that:
    /// - `add_distinct_component_unchecked` inserts the component when a component from an entity with a smaller generation is present.
    /// - it correctly reports the success.
    /// - the component is flagged as inserted.
    /// - the component is not flagged as modified.
    #[test]
    fn add_larger_gen_component() {
        let mut world = World::new();

        let eid1 = world.add_entity(USIZE(1));

        world.run(|mut entities: EntitiesViewMut<'_>| {
            entities.delete_unchecked(eid1);
        });

        let eid2 = world.add_entity(());

        assert_eq!(eid1.index(), eid2.index());

        let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

        let was_inserted = usizes.add_distinct_component_unchecked(eid2, USIZE(0));

        assert!(usizes.get(eid1).is_err());
        assert_eq!(usizes.get(eid2).unwrap(), &USIZE(0));

        assert!(was_inserted);
        assert!(usizes.is_inserted(eid2));
        assert!(!usizes.is_modified(eid2));
    }
}
