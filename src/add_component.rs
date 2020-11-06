use crate::storage::EntityId;
use crate::ViewMut;

/// Defines how components are added to an existing entity.
pub trait AddComponent {
    type Component;
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::add_component`] if you're unsure.  
    ///
    /// ### Example
    /// ```
    /// use shipyard::{World, EntitiesViewMut, ViewMut, AddComponent};
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>();
    /// let entity = entities.add_entity((), ());
    ///
    /// u32s.add_component_unchecked(entity, 0);
    /// ```
    ///
    /// [`Entities::add_component`]: https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_component
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component);
}

impl AddComponent for () {
    type Component = ();

    #[inline]
    fn add_component_unchecked(&mut self, _: EntityId, _: Self::Component) {}
}

impl<T: 'static> AddComponent for ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component) {
        self.insert(entity, component);
    }
}

impl<T: 'static> AddComponent for &mut ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component) {
        self.insert(entity, component);
    }
}

macro_rules! impl_add_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: AddComponent,)+> AddComponent for ($($storage,)+) {
            type Component = ($($storage::Component,)+);

            #[inline]
            fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component) {
                $(
                    self.$index.add_component_unchecked(entity, component.$index);
                )+
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
