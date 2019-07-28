use crate::entity::Key;
use crate::sparse_array::ViewMut;

pub trait ViewAddEntity {
    type Component;
    fn add_entity(self, component: Self::Component, entity: Key);
}

impl ViewAddEntity for () {
    type Component = ();
    fn add_entity(self, _: Self::Component, _: Key) {}
}

macro_rules! impl_view_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: 'static + Send + Sync),+> ViewAddEntity for ($(ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(mut self, component: Self::Component, entity: Key) {
                $(
                    self.$index.insert(entity, component.$index);
                )+
            }
        }
        impl<'a, $($type: 'static + Send + Sync),+> ViewAddEntity for ($(&mut ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entity: Key) {
                $(
                    self.$index.insert(entity, component.$index);
                )+
            }
        }
    }
}

macro_rules! view_add_entity {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_view_add_entity![$(($left_type, $left_index))*];
        view_add_entity![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_view_add_entity![$(($type, $index))*];
    }
}

view_add_entity![(A, 0);  (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
