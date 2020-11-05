use crate::storage::EntityId;
use crate::view::ViewMut;

pub trait AddEntity {
    type Component;

    fn add_entity(&mut self, entity: EntityId, component: Self::Component);
}

impl AddEntity for () {
    type Component = ();

    #[inline]
    fn add_entity(&mut self, _: EntityId, _: Self::Component) {}
}

impl<T: 'static> AddEntity for ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_entity(&mut self, entity: EntityId, component: Self::Component) {
        (&mut &mut *self).add_entity(entity, component);
    }
}

impl<T: 'static> AddEntity for &mut ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_entity(&mut self, entity: EntityId, component: Self::Component) {
        self.insert(entity, component);
    }
}

macro_rules! impl_view_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: AddEntity),+> AddEntity for ($($type,)+) {
            type Component = ($($type::Component,)+);

            #[inline]
            fn add_entity(&mut self, entity: EntityId , components: Self::Component) {
                $(
                    self.$index.add_entity(entity, components.$index);
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
