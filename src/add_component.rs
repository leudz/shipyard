use crate::component::Component;
use crate::entity_id::EntityId;
use crate::ViewMut;

/// Defines how components are added to an existing entity.
pub trait AddComponent<T> {
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
    fn add_component_unchecked(&mut self, entity: EntityId, component: T)
    where
        Self: Sized;
}

impl AddComponent<()> for () {
    #[inline]
    fn add_component_unchecked(&mut self, _: EntityId, _: ()) {}
}

impl<T: Component, TRACK> AddComponent<T> for ViewMut<'_, T, TRACK> {
    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: T) {
        let _ = self.sparse_set.insert(entity, component, self.current);
    }
}

impl<T: Component, TRACK> AddComponent<T> for &mut ViewMut<'_, T, TRACK> {
    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: T) {
        let _ = self.sparse_set.insert(entity, component, self.current);
    }
}

impl<T: Component, TRACK> AddComponent<Option<T>> for ViewMut<'_, T, TRACK> {
    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Option<T>) {
        if let Some(component) = component {
            let _ = self.sparse_set.insert(entity, component, self.current);
        }
    }
}

impl<T: Component, TRACK> AddComponent<Option<T>> for &mut ViewMut<'_, T, TRACK> {
    #[inline]
    #[track_caller]
    fn add_component_unchecked(&mut self, entity: EntityId, component: Option<T>) {
        if let Some(component) = component {
            let _ = self.sparse_set.insert(entity, component, self.current);
        }
    }
}

macro_rules! impl_add_component {
    ($(($storage: ident, $component: ident, $index: tt))+) => {
        impl<$($component: Component,)+ $($storage: AddComponent<$component>,)+> AddComponent<($($component,)+)> for ($($storage,)+) {
            #[inline]
            #[track_caller]
            fn add_component_unchecked(&mut self, entity: EntityId, component: ($($component,)+)) {
                $(
                    let _ = self.$index.add_component_unchecked(entity, component.$index);
                )+
            }
        }
    }
}

macro_rules! add_component {
    ($(($storage: ident, $component: ident, $index: tt))+; ($storage1: ident, $component1: ident, $index1: tt) $(($queue_storage: ident, $queue_component: ident, $queue_index: tt))*) => {
        impl_add_component![$(($storage, $component, $index))*];
        add_component![$(($storage, $component, $index))* ($storage1, $component1, $index1); $(($queue_storage, $queue_component, $queue_index))*];
    };
    ($(($storage: ident, $component: ident, $index: tt))+;) => {
        impl_add_component![$(($storage, $component, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
add_component![(ViewA, A, 0); (ViewB, B, 1) (ViewC, C, 2) (ViewD, D, 3) (ViewE, E, 4) (ViewF, F, 5) (ViewG, G, 6) (ViewH, H, 7) (ViewI, I, 8) (ViewJ, J, 9)];
#[cfg(feature = "extended_tuple")]
add_component![
    (ViewA, A, 0); (ViewB, B, 1) (ViewC, C, 2) (ViewD, D, 3) (ViewE, E, 4) (ViewF, F, 5) (ViewG, G, 6) (ViewH, H, 7) (ViewI, I, 8) (ViewJ, J, 9)
    (ViewK, K, 10) (ViewL, L, 11) (ViewM, M, 12) (ViewN, N, 13) (ViewO, O, 14) (ViewP, P, 15) (ViewQ, Q, 16) (ViewR, R, 17) (ViewS, S, 18) (ViewT, T, 19)
    (ViewU, U, 20) (ViewV, V, 21) (ViewW, W, 22) (ViewX, X, 23) (ViewY, Y, 24) (ViewZ, Z, 25) (ViewAA, AA, 26) (ViewBB, BB, 27) (ViewCC, CC, 28) (ViewDD, DD, 29)
    (ViewEE, EE, 30) (ViewFF, FF, 31)
];
