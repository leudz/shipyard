use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::storage::StorageId;
use crate::view::{ViewInfo, ViewMut};
use tinyvec::TinyVec;

pub trait AddEntity {
    type Component;

    fn add_entity(self, entity: EntityId, component: Self::Component, component_list: &[StorageId]);
}

impl AddEntity for () {
    type Component = ();

    #[inline]
    fn add_entity(self, _: EntityId, _: Self::Component, _: &[StorageId]) {}
}

impl<T: 'static> AddEntity for ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_entity(
        mut self,
        entity: EntityId,
        component: Self::Component,
        component_list: &[StorageId],
    ) {
        (&mut self).add_entity(entity, component, component_list);
    }
}

impl<T: 'static> AddEntity for &mut ViewMut<'_, T> {
    type Component = T;

    #[inline]
    fn add_entity(
        self,
        entity: EntityId,
        component: Self::Component,
        component_list: &[StorageId],
    ) {
        self.insert(component, entity);

        match &self.metadata.pack {
            Pack::Tight(tight) => {
                if tight.is_storage_packable(component_list) {
                    self.pack(entity);
                }
            }
            Pack::Loose(loose) => {
                if loose.is_storage_packable(component_list) {
                    self.pack(entity);
                }
            }
            Pack::None => {}
        }
    }
}

impl<T: 'static> AddEntity for (ViewMut<'_, T>,) {
    type Component = (T,);

    #[inline]
    fn add_entity(
        self,
        entity: EntityId,
        component: Self::Component,
        component_list: &[StorageId],
    ) {
        self.0.add_entity(entity, component.0, component_list);
    }
}

impl<T: 'static> AddEntity for (&mut ViewMut<'_, T>,) {
    type Component = (T,);

    #[inline]
    fn add_entity(
        self,
        entity: EntityId,
        component: Self::Component,
        component_list: &[StorageId],
    ) {
        self.0.add_entity(entity, component.0, component_list);
    }
}

macro_rules! impl_view_add_entity {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: AddEntity + ViewInfo),+> AddEntity for ($($type,)+) {
            type Component = ($($type::Component,)+);

            #[inline]
            fn add_entity(self, entity: EntityId , components: Self::Component, _: &[StorageId]) {
                let mut storage_info = TinyVec::new();

                $(
                    $type::storage_info(&mut storage_info);
                )+

                storage_info.sort_unstable();

                $(
                    self.$index.add_entity(entity, components.$index, &storage_info);
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
