use crate::entity::Key;
use crate::sparse_array::{SparseArray, ViewMut, Write};

// No new storage will be created
pub trait AddComponent {
    type Component;
    /// Stores `component` in `entity`, if the entity had already a component
    /// of this type, it will be replaced.
    ///
    /// Multiple components can be added at the same time using a tuple.
    fn add_component(self, component: Self::Component, entity: Key);
}

impl<T> AddComponent for &mut SparseArray<T> {
    type Component = T;
    fn add_component(self, component: Self::Component, key: Key) {
        self.insert(key.index(), component);
    }
}

impl<T> AddComponent for &mut ViewMut<'_, T> {
    type Component = T;
    fn add_component(self, component: Self::Component, key: Key) {
        self.insert(key, component);
    }
}

macro_rules! impl_add_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type),+> AddComponent for ($(&mut SparseArray<$type>,)+) {
            type Component = ($($type,)+);
            fn add_component(self, component: Self::Component, key: Key) {
                $(
                    AddComponent::add_component(self.$index, component.$index, key);
                )+
            }
        }
        impl<$($type),+> AddComponent for ($(Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_component(mut self, component: Self::Component, key: Key) {
                $(
                    <&mut SparseArray<$type>>::add_component(&mut *self.$index, component.$index, key);
                )+
            }
        }
        impl<$($type),+> AddComponent for ($(&mut Write<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_component(self, component: Self::Component, key: Key) {
                $(
                    <&mut SparseArray<$type>>::add_component(&mut *self.$index, component.$index, key);
                )+
            }
        }
        impl<$($type),+> AddComponent for ($(&mut ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_component(self, component: Self::Component, key: Key) {
                $(
                    <&mut ViewMut<$type>>::add_component(&mut *self.$index, component.$index, key);
                )+
            }
        }
    }
}

macro_rules! add_component {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_add_component![$(($left_type, $left_index))*];
        add_component![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_add_component![$(($type, $index))*];
    }
}

add_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
