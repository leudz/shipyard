use crate::error;
use crate::sparse_array::{SparseArray, Write};
use std::any::TypeId;
use std::sync::Arc;

/// Allows to pack multiple storages.
pub trait OwnedPack {
    /// Pack multiple storages together, it can speed up iteration at the cost of insertion/removal.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
    /// (&mut usizes, &mut u32s).try_pack_owned().unwrap();
    /// ```
    fn try_pack_owned(self) -> Result<(), error::Pack>;
    /// Pack multiple storages together, it can speed up iteration at the cost of insertion/removal.
    ///
    /// Unwraps errors.
    /// # Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
    /// (&mut usizes, &mut u32s).pack_owned();
    /// ```
    fn pack_owned(self);
}

macro_rules! impl_owned_pack {
    ($(($type: ident, $index: tt))+) => {
        #[allow(clippy::useless_let_if_seq)]
        impl<$($type: 'static),+> OwnedPack for ($(&mut SparseArray<$type>,)+) {
            fn try_pack_owned(self) -> Result<(), error::Pack> {
                $(
                    if self.$index.is_packed_owned() {
                        return Err(error::Pack::AlreadyPacked(TypeId::of::<$type>()));
                    }
                )+

                let mut type_ids = vec![$(TypeId::of::<$type>()),+];
                type_ids.sort_unstable();
                let type_ids: Arc<_> = type_ids.into_boxed_slice().into();

                $(
                    self.$index.pack_with(Arc::clone(&type_ids));
                )+

                let mut smallest = std::usize::MAX;
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

                let mut indices = vec![];

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
            fn pack_owned(self) {
                self.try_pack_owned().unwrap()
            }
        }

        impl<$($type: 'static),+> OwnedPack for ($(Write<'_, $type>,)+) {
            fn try_pack_owned(mut self) -> Result<(), error::Pack> {
                ($(&mut *self.$index,)+).try_pack_owned()
            }
            fn pack_owned(self) {
                self.try_pack_owned().unwrap()
            }
        }

        impl<$($type: 'static),+> OwnedPack for ($(&mut Write<'_, $type>,)+) {
            fn try_pack_owned(self) -> Result<(), error::Pack> {
                ($(&mut **self.$index,)+).try_pack_owned()
            }
            fn pack_owned(self) {
                self.try_pack_owned().unwrap()
            }
        }
    }
}

macro_rules! owned_pack {
    ($(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_owned_pack![$(($left_type, $left_index))*];
        owned_pack![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_owned_pack![$(($type, $index))*];
    }
}

owned_pack![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
