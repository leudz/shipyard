use super::SparseSet;
use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::views::ViewMut;
use std::any::TypeId;

pub trait ViewAddEntity {
    type Component;
    fn add_entity(self, component: Self::Component, entity: EntityId);
}

impl ViewAddEntity for () {
    type Component = ();
    fn add_entity(self, _: Self::Component, _: EntityId) {}
}

impl<T: 'static> ViewAddEntity for &mut ViewMut<'_, T> {
    type Component = T;
    fn add_entity(self, component: Self::Component, entity: EntityId) {
        self.insert(component, entity);

        let SparseSet {
            sparse,
            dense,
            data,
            pack_info,
        } = &mut **self;

        if let Pack::Update(pack) = &mut pack_info.pack {
            let len = dense.len() - 1;
            unsafe {
                *sparse.get_unchecked_mut(entity.index()) = pack.inserted;
                *sparse.get_unchecked_mut(
                    dense.get_unchecked(pack.inserted + pack.modified).index(),
                ) = len;
                *sparse.get_unchecked_mut(dense.get_unchecked(pack.inserted).index()) =
                    pack.inserted + pack.modified;
            }
            dense.swap(pack.inserted + pack.modified, len);
            dense.swap(pack.inserted, pack.inserted + pack.modified);
            data.swap(pack.inserted + pack.modified, len);
            data.swap(pack.inserted, pack.inserted + pack.modified);
            pack.inserted += 1;
        }
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
        impl<'a, $($type: 'static + Send + Sync),+> ViewAddEntity for ($(&mut ViewMut<'_, $type>,)+) {
            type Component = ($($type,)+);
            fn add_entity(self, component: Self::Component, entity: EntityId) {
                let sparse_sets = ($(&mut **self.$index,)+);

                $(
                    sparse_sets.$index.insert(component.$index, entity);
                )+

                let mut type_ids = [$(TypeId::of::<$type>()),+];
                type_ids.sort_unstable();

                let mut should_pack = Vec::with_capacity(type_ids.len());
                $(
                    let type_id = TypeId::of::<$type>();

                    if should_pack.contains(&type_id) {
                        sparse_sets.$index.pack(entity);
                    } else {
                        match &mut sparse_sets.$index.pack_info.pack {
                            Pack::Tight(pack) => if let Ok(types) = pack.check_types(&type_ids) {
                                should_pack.extend_from_slice(&pack.types);
                                if !types.is_empty() {
                                    sparse_sets.$index.pack(entity);
                                }
                            }
                            Pack::Loose(pack) => if let Ok(types) = pack.check_all_types(&type_ids) {
                                should_pack.extend_from_slice(&pack.tight_types);
                                if !types.is_empty() {
                                    sparse_sets.$index.pack(entity);
                                }
                            }
                            Pack::Update(pack) => {
                                let len = sparse_sets.$index.dense.len() - 1;
                                unsafe {
                                    *sparse_sets.$index.sparse.get_unchecked_mut(entity.index()) = pack.inserted;
                                    *sparse_sets.$index.sparse.get_unchecked_mut(sparse_sets.$index.dense.get_unchecked(pack.inserted + pack.modified).index()) = len;
                                    *sparse_sets.$index.sparse.get_unchecked_mut(sparse_sets.$index.dense.get_unchecked(pack.inserted).index()) = pack.inserted + pack.modified;
                                }
                                sparse_sets.$index.dense.swap(pack.inserted + pack.modified, len);
                                sparse_sets.$index.dense.swap(pack.inserted, pack.inserted + pack.modified);
                                sparse_sets.$index.data.swap(pack.inserted + pack.modified, len);
                                sparse_sets.$index.data.swap(pack.inserted, pack.inserted + pack.modified);
                                pack.inserted += 1;
                            }
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
