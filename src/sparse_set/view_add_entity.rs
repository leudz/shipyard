use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::view::ViewMut;
use alloc::vec::Vec;
use core::any::TypeId;

pub trait ViewAddEntity {
    type Component;
    fn add_entity(self, component: Self::Component, entity: EntityId);
}

impl ViewAddEntity for () {
    type Component = ();
    fn add_entity(self, _: Self::Component, _: EntityId) {}
}

impl<T: 'static> ViewAddEntity for ViewMut<'_, T> {
    type Component = T;
    fn add_entity(mut self, component: Self::Component, entity: EntityId) {
        self.insert(component, entity);
    }
}

impl<T: 'static> ViewAddEntity for &mut ViewMut<'_, T> {
    type Component = T;
    fn add_entity(self, component: Self::Component, entity: EntityId) {
        self.insert(component, entity);
    }
}

impl<T: 'static> ViewAddEntity for (ViewMut<'_, T>,) {
    type Component = (T,);
    fn add_entity(self, component: Self::Component, entity: EntityId) {
        self.0.add_entity(component.0, entity);
    }
}

impl<T: 'static> ViewAddEntity for (&mut ViewMut<'_, T>,) {
    type Component = (T,);
    fn add_entity(self, component: Self::Component, entity: EntityId) {
        self.0.add_entity(component.0, entity);
    }
}

macro_rules! impl_view_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: 'static),+> ViewAddEntity for ($(ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(mut self, component: Self::Component, entity: EntityId) {
                ($(&mut self.$index),+).add_entity(component, entity)
            }
        }

        impl<'a, $($type: 'static),+> ViewAddEntity for ($(&mut ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entity: EntityId) {
                let sparse_sets = ($(&mut **self.$index,)+);

                $(
                    sparse_sets.$index.insert(component.$index, entity);
                )+

                let type_ids = [$(TypeId::of::<$type>()),+];
                let mut sorted_type_ids = type_ids.clone();
                sorted_type_ids.sort_unstable();

                let mut should_pack = Vec::with_capacity(type_ids.len());
                $(
                    let type_id = type_ids[$index];

                    if should_pack.contains(&type_id) {
                        sparse_sets.$index.pack(entity);
                    } else {
                        match &mut sparse_sets.$index.metadata.pack {
                            Pack::Tight(pack) => if let Ok(types) = pack.is_packable(&sorted_type_ids) {
                                if !types.is_empty() {
                                    should_pack.extend_from_slice(&types);
                                    sparse_sets.$index.pack(entity);
                                }
                            }
                            Pack::Loose(pack) => if let Ok(types) = pack.is_packable(&sorted_type_ids) {
                                if !types.is_empty() {
                                    should_pack.extend_from_slice(&types);
                                    sparse_sets.$index.pack(entity);
                                }
                            }
                            Pack::Update(_) => {}
                            Pack::NoPack => {}
                        }
                    }
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

view_add_entity![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
