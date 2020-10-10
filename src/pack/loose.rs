use crate::borrow::ViewMut;
use crate::error;
use crate::iter::{IntoIter, IntoWithId};
use crate::sparse_set::{LoosePack as LoosePackInfo, Pack, SparseSet};
use crate::type_id::TypeId;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::type_name;

/// Trait used to loose pack storage(s).
pub trait LoosePack<T> {
    /// Loose packs storages.  
    /// A loose pack takes at least a non packed storage and an additional storage in any state.  
    /// A tuple is needed to specify which storages are going to be modified. It always have to be a tuple, even when there's just a single storage.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{LoosePack, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///     usizes.update_pack();
    ///     LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes)).unwrap();
    /// });
    /// ```
    fn try_loose_pack(self) -> Result<(), error::Pack>;
    /// Loose packs storages.  
    /// A loose pack takes at least a non packed storage and an additional storage in any state.  
    /// A tuple is needed to specify which storages are going to be modified. It always have to be a tuple, even when there's just a single storage.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{LoosePack, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(|mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///     usizes.update_pack();
    ///     LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes));
    /// });
    /// ```
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    fn loose_pack(self);
}

macro_rules! impl_loose_pack {
    ($(($tight: ident, $tight_index: tt))+; $(($loose: ident, $loose_index: tt))+) => {
        impl<$($tight: 'static,)+ $($loose: 'static),+> LoosePack<($($tight,)+)> for ($(&mut ViewMut<'_, $tight>,)+ $(&mut ViewMut<'_, $loose>,)+) {
            fn try_loose_pack(self) -> Result<(), error::Pack> {
                // we check if any of the future tightly packed storages are already packed
                $(
                    match self.$tight_index.metadata.pack {
                        Pack::Tight(_) => {
                            return Err(error::Pack::AlreadyTightPack(type_name::<$tight>()));
                        },
                        Pack::Loose(_) => {
                            return Err(error::Pack::AlreadyLoosePack(type_name::<$tight>()));
                        },
                        Pack::None => {}
                    }
                )+

                // we gather and sort all TypeId
                let mut tight_types: Box<[_]> = Box::new([$(TypeId::of::<SparseSet<$tight>>()),+]);
                tight_types.sort_unstable();
                let tight_types: Arc<[_]> = tight_types.into();

                let mut loose_types: Box<[_]> = Box::new([$(TypeId::of::<SparseSet<$loose>>()),+]);
                loose_types.sort_unstable();
                let loose_types: Arc<[_]> = loose_types.into();

                // make tightly packed storages loose packed
                $(
                    self.$tight_index.metadata.pack = Pack::Loose(
                        LoosePackInfo::new(Arc::clone(&tight_types), Arc::clone(&loose_types))
                    );
                )+

                // add tightly packed storages's TypeId to the list of observer in loose storages
                $(
                    for tight_type in tight_types.iter().copied() {
                        match self
                            .$loose_index
                            .metadata
                            .observer_types
                            .binary_search(&tight_type) {
                                Ok(_) => {},
                                Err(index) => self
                                    .$loose_index
                                    .metadata
                                    .observer_types
                                    .insert(index, tight_type),
                        }
                    }
                )+

                // using an iterator we get all entities with all components
                let entities: Vec<_> = ($(&mut *self.$tight_index,)+ $(&mut *self.$loose_index,)+).iter().ids().collect();

                // we then use this list to pack the entity
                for entity in entities {
                    $(
                        self.$tight_index.pack(entity);
                    )+
                }

                Ok(())
            }
            #[cfg(feature = "panic")]
            #[track_caller]
            fn loose_pack(self) {
                match LoosePack::<($($tight,)+)>::try_loose_pack(self) {
                    Ok(_) => (),
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }
}

macro_rules! loose_pack {
    ($(($tight: ident, $tight_index: tt))+; ($loose1: ident, $loose_index1: tt) $(($loose: ident, $loose_index: tt))+; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_loose_pack![$(($tight, $tight_index))+; ($loose1, $loose_index1) $(($loose, $loose_index))+];
        loose_pack![$(($tight, $tight_index))+ ($loose1, $loose_index1); $(($loose, $loose_index))+; $(($queue_type, $queue_index))*];
    };
    (($tight1: ident, $tight_index1: tt) $(($tight: ident, $tight_index: tt))*; ($loose1: ident, $loose_index1: tt); ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_loose_pack![($tight1, $tight_index1) $(($tight, $tight_index))*; ($loose1, $loose_index1)];
        loose_pack![($tight1, $tight_index1); $(($tight, $tight_index))* ($loose1, $loose_index1) ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    (($tight1: ident, $tight_index1: tt) $(($tight: ident, $tight_index: tt))*; ($loose1: ident, $loose_index1: tt);) => {
        impl_loose_pack![($tight1, $tight_index1) $(($tight, $tight_index))*; ($loose1, $loose_index1)];
    };
    ($(($tight: ident, $tight_index: tt))+;;) => {}
}

loose_pack![(A, 0); (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
