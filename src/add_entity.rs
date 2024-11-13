use crate::component::Component;
use crate::entity_id::EntityId;
use crate::views::ViewMut;

/// Trait describing how to add a new entity to a storage.
pub trait AddEntity {
    #[allow(missing_docs)]
    type Component;

    /// Adds a new entity with `component`.
    fn add_entity(storage: &mut Self, entity: EntityId, component: Self::Component);
}

impl AddEntity for () {
    type Component = ();

    #[inline]
    fn add_entity(_: &mut Self, _: EntityId, _: Self::Component) {}
}

impl<T: Component, TRACK> AddEntity for ViewMut<'_, T, TRACK> {
    type Component = T;

    #[inline]
    fn add_entity(storage: &mut Self, entity: EntityId, component: Self::Component) {
        AddEntity::add_entity(&mut &mut *storage, entity, component);
    }
}

impl<T: Component, TRACK> AddEntity for &mut ViewMut<'_, T, TRACK> {
    type Component = T;

    #[inline]
    #[track_caller]
    fn add_entity(storage: &mut Self, entity: EntityId, component: Self::Component) {
        let _ = storage
            .sparse_set
            .insert(entity, component, storage.current);
    }
}

macro_rules! impl_view_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: AddEntity),+> AddEntity for ($($type,)+) {
            type Component = ($($type::Component,)+);

            #[inline]
            fn add_entity(storages: &mut Self, entity: EntityId , components: Self::Component) {
                $(
                    AddEntity::add_entity(&mut storages.$index, entity, components.$index);
                )+
            }
        }
    }
}

macro_rules! view_add_entity {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_view_add_entity![$(($type, $index))*];
        view_add_entity![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_view_add_entity![$(($type, $index))*];
    }
}

view_add_entity![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
