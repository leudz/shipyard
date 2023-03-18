use crate::component::Component;
use crate::entity_id::EntityId;
use crate::ViewMut;

/// Defines how components are added to an existing entity.
pub trait AddComponent {
    #[allow(missing_docs)]
    type Component;
    /// Adds `component` to `entity`, multiple components can be added at the same time using a tuple.  
    /// This function does not check `entity` is alive. It's possible to add components to removed entities.  
    /// Use [`Entities::add_component`] if you're unsure.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{AddComponent, Component, EntitiesViewMut, ViewMut, World};
    ///
    /// #[derive(Component)]
    /// struct U32(u32);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();
    /// let entity = entities.add_entity((), ());
    ///
    /// u32s.add_component_unchecked(entity, U32(0));
    /// ```
    ///
    /// [`Entities::add_component`]: crate::Entities::add_component()
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component);
}

impl AddComponent for () {
    type Component = ();

    #[inline]
    fn add_component_unchecked(&mut self, _: EntityId, _: Self::Component) {}
}

impl<T: Component, TRACK> AddComponent for ViewMut<'_, T, TRACK> {
    type Component = T;

    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component) {
        self.sparse_set.insert(entity, component, self.current);
    }
}

impl<T: Component, TRACK> AddComponent for &mut ViewMut<'_, T, TRACK> {
    type Component = T;

    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Self::Component) {
        self.sparse_set.insert(entity, component, self.current);
    }
}

macro_rules! impl_add_component {
    ($(($storage: ident, $index: tt))+) => {
        impl<$($storage: AddComponent,)+> AddComponent for ($($storage,)+) {
            type Component = ($($storage::Component,)+);

            #[inline]
            #[track_caller]
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
