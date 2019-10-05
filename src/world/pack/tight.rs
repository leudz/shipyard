use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::atomic_refcell::{Ref, RefMut};
use crate::component_storage::AllStorages;
use crate::error;
use crate::sparse_array::{Pack, SparseArray, TightPack as TightPackInfo};
use std::any::TypeId;
use std::sync::Arc;

pub trait TightPack {
    fn try_tight_pack(all_storages: &AtomicRefCell<AllStorages>) -> Result<(), error::Pack>;
}

macro_rules! impl_tight_pack {
    ($(($type: ident, $index: tt))+) => {
        #[allow(clippy::useless_let_if_seq)]
        impl<$($type: 'static),+> TightPack for ($($type,)+) {
            fn try_tight_pack(all_storages: &AtomicRefCell<AllStorages>) -> Result<(), error::Pack> {
                let all_storages = all_storages.try_borrow().map_err(error::GetStorage::AllStoragesBorrow)?;

                let mut type_ids: Box<[_]> = Box::new([$(TypeId::of::<$type>(),)+]);
                let mut storages: ($((RefMut<SparseArray<$type>>, Borrow),)+) = ($({
                    // SAFE borrow is dropped after storage
                    let (storage, borrow) = unsafe {Ref::destructure(Ref::try_map(Ref::clone(&all_storages), |all_storages| {
                        match all_storages.0.get(&type_ids[$index]) {
                            Some(storage) => Ok(storage),
                            None => Err(error::GetStorage::MissingComponent),
                        }
                    })?)};
                    (storage.array_mut()
                    .map_err(|err| error::Pack::GetStorage(error::GetStorage::StorageBorrow(err)))?, borrow)
                },)+);

                type_ids.sort_unstable();
                let type_ids: Arc<[_]> = type_ids.into();

                $(
                    if storages.$index.0.is_unique() {
                        return Err(error::Pack::UniqueStorage(std::any::type_name::<$type>()));
                    }
                )+

                $(
                    match storages.$index.0.pack_info.pack {
                        Pack::Tight(_) => {
                            return Err(error::Pack::AlreadyTightPack(TypeId::of::<$type>()));
                        },
                        Pack::Loose(_) => {
                            return Err(error::Pack::AlreadyLoosePack(TypeId::of::<$type>()));
                        },
                        Pack::Update(_) => {
                            return Err(error::Pack::AlreadyUpdatePack(TypeId::of::<$type>()))
                        },
                        Pack::NoPack => {
                            storages.$index.0.pack_info.pack = Pack::Tight(TightPackInfo::new(Arc::clone(&type_ids)));
                        }
                    }
                )+

                let mut smallest = std::usize::MAX;
                let mut smallest_index = 0;
                let mut i = 0;

                $(
                    if storages.$index.0.len() < smallest {
                        smallest = storages.$index.0.len();
                        smallest_index = i;
                    }
                    i += 1;
                )+
                let _ = smallest;
                let _ = i;

                let mut indices = vec![];

                $(
                    if $index == smallest_index {
                        indices = storages.$index.0.clone_indices();
                    }
                )+

                for index in indices {
                    $(
                        if !storages.$index.0.contains(index) {
                            continue
                        }
                    )+
                    $(
                        storages.$index.0.pack(index);
                    )+
                }

                Ok(())
            }
        }
    }
}

macro_rules! tight_pack {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_tight_pack![$(($type, $index))*];
        tight_pack![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_tight_pack![$(($type, $index))*];
    }
}

tight_pack![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
