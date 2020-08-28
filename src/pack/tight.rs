use crate::error;
use crate::sparse_set::{Pack, TightPack as TightPackInfo};
use crate::type_id::TypeId;
use crate::view::ViewMut;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::type_name;

/// Trait used to tight pack storage(s).
pub trait TightPack {
    /// Tight packs storages.  
    /// Only non packed storages can be tight packed at the moment.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{TightPack, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world
    ///     .borrow::<(ViewMut<usize>, ViewMut<u32>)>()
    ///     .try_tight_pack()
    ///     .unwrap();
    /// ```
    fn try_tight_pack(self) -> Result<(), error::Pack>;
    /// Tight packs storages.  
    /// Only non packed storages can be tight packed at the moment.  
    /// Unwraps errors.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{TightPack, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world
    ///     .borrow::<(ViewMut<usize>, ViewMut<u32>)>()
    ///     .tight_pack();
    /// ```
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    fn tight_pack(self);
}

macro_rules! impl_tight_pack {
    ($(($type: ident, $index: tt))+) => {
        #[allow(clippy::useless_let_if_seq)]
        impl<$($type: 'static),+> TightPack for ($(&mut ViewMut<'_, $type>,)+) {
            fn try_tight_pack(self) -> Result<(), error::Pack> {
                let mut type_ids: Box<[_]> = Box::new([$(TypeId::of::<$type>(),)+]);

                type_ids.sort_unstable();
                let type_ids: Arc<[_]> = type_ids.into();

                $(
                    match self.$index.metadata.pack {
                        Pack::Tight(_) => {
                            return Err(error::Pack::AlreadyTightPack(type_name::<$type>()));
                        },
                        Pack::Loose(_) => {
                            return Err(error::Pack::AlreadyLoosePack(type_name::<$type>()));
                        },
                        Pack::Update(_) => {
                            return Err(error::Pack::AlreadyUpdatePack(type_name::<$type>()))
                        },
                        Pack::NoPack => {
                            self.$index.metadata.pack = Pack::Tight(TightPackInfo::new(Arc::clone(&type_ids)));
                        }
                    }
                )+

                let mut smallest = core::usize::MAX;
                let mut smallest_index = 0;
                let mut i = 0;

                $(
                    if self.$index.len() < smallest {
                        smallest = self.$index.len();
                        smallest_index = i;
                    }
                    i += 1;
                )+
                let _ = smallest;
                let _ = i;

                let mut indices = Vec::new();

                $(
                    if $index == smallest_index {
                        indices = self.$index.clone_indices();
                    }
                )+

                for index in indices {
                    $(
                        if !self.$index.contains(index) {
                            continue
                        }
                    )+
                    $(
                        self.$index.pack(index);
                    )+
                }

                Ok(())
            }
                #[cfg(feature = "panic")]
                fn tight_pack(self) {
                self.try_tight_pack().unwrap()
            }
        }
        impl<$($type: 'static),+> TightPack for ($(ViewMut<'_, $type>,)+) {
            fn try_tight_pack(mut self) -> Result<(), error::Pack> {
                ($(&mut self.$index,)+).try_tight_pack()
            }
            #[cfg(feature = "panic")]
            #[track_caller]
            fn tight_pack(self) {
                match self.try_tight_pack() {
                    Ok(_) => (),
                    Err(err) => panic!("{:?}", err),
                }
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
