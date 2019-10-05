use crate::atomic_refcell::{AtomicRefCell, Borrow};
use crate::atomic_refcell::{Ref, RefMut};
use crate::component_storage::AllStorages;
use crate::error;
use crate::sparse_array::{LoosePack as LoosePackInfo, Pack, SparseArray};
use std::any::TypeId;
use std::sync::Arc;

pub trait LoosePack {
    fn try_loose_pack(all_storages: &AtomicRefCell<AllStorages>) -> Result<(), error::Pack>;
}

macro_rules! impl_loose_pack {
    ($(($tight: ident, $tight_index: tt))+; $(($loose: ident, $loose_index: tt))+) => {
        #[allow(clippy::useless_let_if_seq)]
        impl<$($tight: 'static,)+ $($loose: 'static),+> LoosePack for (($($tight,)+), ($($loose,)+)) {
            fn try_loose_pack(all_storages: &AtomicRefCell<AllStorages>) -> Result<(), error::Pack> {
                let all_storages = all_storages.try_borrow().map_err(error::GetStorage::AllStoragesBorrow)?;

                let mut tight_types: Box<[_]> = Box::new([$(TypeId::of::<$tight>()),+]);
                let mut loose_types: Box<[_]> = Box::new([$(TypeId::of::<$loose>()),+]);
                let mut storages: ($((RefMut<SparseArray<$tight>>, Borrow),)+ $((RefMut<SparseArray<$loose>>, Borrow),)+) = ($({
                    // SAFE borrow is dropped after storage
                    let (storage, borrow) = unsafe {Ref::destructure(Ref::try_map(Ref::clone(&all_storages), |all_storages| {
                        match all_storages.0.get(&tight_types[$tight_index]) {
                            Some(storage) => Ok(storage),
                            None => Err(error::GetStorage::MissingComponent),
                        }
                    })?)};
                    (storage.array_mut()
                    .map_err(|err| error::Pack::GetStorage(error::GetStorage::StorageBorrow(err)))?, borrow)
                },)+
                $({
                    // SAFE borrow is dropped after storage
                    let (storage, borrow) = unsafe {Ref::destructure(Ref::try_map(Ref::clone(&all_storages), |all_storages| {
                        match all_storages.0.get(&loose_types[$loose_index - tight_types.len()]) {
                            Some(storage) => Ok(storage),
                            None => Err(error::GetStorage::MissingComponent),
                        }
                    })?)};
                    (storage.array_mut()
                    .map_err(|err| error::Pack::GetStorage(error::GetStorage::StorageBorrow(err)))?, borrow)
                },)+
                );

                tight_types.sort_unstable();
                loose_types.sort_unstable();
                let tight_types: Arc<[_]> = tight_types.into();
                let loose_types: Arc<[_]> = loose_types.into();

                $(
                    if storages.$tight_index.0.is_unique() {
                        return Err(error::Pack::UniqueStorage(std::any::type_name::<$tight>()));
                    }
                )+

                $(
                    if storages.$loose_index.0.is_unique() {
                        return Err(error::Pack::UniqueStorage(std::any::type_name::<$loose>()));
                    }
                )+

                $(
                    match storages.$tight_index.0.pack_info.pack {
                        Pack::Tight(_) => {
                            return Err(error::Pack::AlreadyTightPack(TypeId::of::<$tight>()));
                        },
                        Pack::Loose(_) => {
                            return Err(error::Pack::AlreadyLoosePack(TypeId::of::<$tight>()));
                        },
                        Pack::Update(_) => {
                            return Err(error::Pack::AlreadyUpdatePack(TypeId::of::<$tight>()))
                        },
                        Pack::NoPack => {
                            storages.$tight_index.0.pack_info.pack = Pack::Loose(LoosePackInfo::new(Arc::clone(&tight_types), Arc::clone(&loose_types)));
                        }
                    }
                )+

                $(
                    for tight_type in tight_types.iter().copied() {
                        match storages.$loose_index.0.pack_info.observer_types.binary_search(&tight_type) {
                            Ok(_) => {},
                            Err(index) => storages.$loose_index.0.pack_info.observer_types.insert(index, tight_type),
                        }
                    }
                )+

                let mut smallest = std::usize::MAX;
                let mut smallest_index = 0;
                let mut i = 0;

                $(
                    if storages.$tight_index.0.len() < smallest {
                        smallest = storages.$tight_index.0.len();
                        smallest_index = i;
                    }
                    i += 1;
                )+

                $(
                    if storages.$loose_index.0.len() < smallest {
                        smallest = storages.$loose_index.0.len();
                        smallest_index = i;
                    }
                    i += 1;
                )+

                let _ = (smallest, i);

                let mut indices: Vec<_> = Vec::new();

                $(
                    if $tight_index == smallest_index {
                        indices = storages.$tight_index.0.clone_indices();
                    }
                )+

                $(
                    if $loose_index == smallest_index {
                        indices = storages.$loose_index.0.clone_indices();
                    }
                )+

                for index in indices {
                    $(
                        if !storages.$tight_index.0.contains(index) {
                            continue
                        }
                    )+

                    $(
                        if !storages.$loose_index.0.contains(index) {
                            continue
                        }
                    )+

                    $(
                        storages.$tight_index.0.pack(index);
                    )+
                }

                Ok(())
            }
        }
    }
}

macro_rules! loose_pack {
    ($(($tight: ident, $tight_index: tt))+; ($loose1: ident, $loose_index1: tt) $(($loose: ident, $loose_index: tt))+; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_loose_pack![$(($tight, $tight_index))+; ($loose1, $loose_index1) $(($loose, $loose_index))+];
        loose_pack![$(($tight, $tight_index))+ ($loose1, $loose_index1); $(($loose, $loose_index))+; $(($queue_type, $queue_index))*];
    };
    (($tight1: ident, $tight_index1: tt) $(($tight: ident, $tight_index: tt))*; ($loose: ident, $loose_index: tt); ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_loose_pack![($tight1, $tight_index1) $(($tight, $tight_index))*; ($loose, $loose_index)];
        loose_pack![($tight1, $tight_index1); $(($tight, $tight_index))* ($loose, $loose_index) ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    ($(($tight: ident, $tight_index: tt))+; ($loose: ident, $loose_index: tt);) => {
        impl_loose_pack![$(($tight, $tight_index))+; ($loose, $loose_index)];
    }
}

loose_pack![(A, 0); (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
