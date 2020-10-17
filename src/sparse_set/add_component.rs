use crate::storage::EntityId;
use crate::view::ViewMut;

/// Adds components to an existing entity.
pub trait AddComponentUnchecked<T> {
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::try_add_component`] if you're unsure.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{World, EntitiesViewMut, ViewMut, AddComponentUnchecked};
    ///
    /// let world = World::new();
    ///
    /// let entity = world.borrow::<EntitiesViewMut>().add_entity((), ());
    ///
    /// world.run(|mut u32s: ViewMut<u32>| {
    ///     u32s.try_add_component_unchecked(0, entity).unwrap();
    /// });
    /// ```
    ///
    /// [`Entities::try_add_component`]: https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.try_add_component
    fn add_component_unchecked(
        self,
        component: T,
        entity: EntityId,
    );
}

impl<T: 'static> AddComponentUnchecked<T> for &mut ViewMut<'_, T> {
    #[inline]
    fn add_component_unchecked(
        self,
        component: T,
        entity: EntityId,
    ) {
        self.insert(component, entity);
    }
}

macro_rules! impl_add_component_unchecked {
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponentUnchecked<($($type,)+)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            #[inline]
            fn add_component_unchecked(self, component: ($($type,)+), entity: EntityId) {
                $(
                    self.$index.insert(component.$index, entity);
                )+
            }
        }
    }
}

macro_rules! add_component_unchecked {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component_unchecked![($type1, $index1) $(($type, $index))*;];
        add_component_unchecked![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component_unchecked![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        add_component_unchecked![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_add_component_unchecked![$(($type, $index))+;];
    }
}

add_component_unchecked![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
