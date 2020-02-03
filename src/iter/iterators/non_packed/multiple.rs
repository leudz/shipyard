use super::super::update::*;
use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{Folder, UnindexedProducer};

macro_rules! impl_iterators {
    (
        $number: literal
        $non_packed: ident
        $update: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Non packed iterator over"]
        #[doc = $number]
        #[doc = "components.\n"]
        pub struct $non_packed<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) indices: *const EntityId,
            pub(crate) current: usize,
            pub(crate) end: usize,
            pub(crate) array: usize,
        }

        unsafe impl<$($type: IntoAbstract),+> Send for $non_packed<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {}

        impl<$($type: IntoAbstract),+> From<$update<$($type),+>> for $non_packed<$($type),+> {
            fn from(update: $update<$($type),+>) -> Self {
                $non_packed {
                    data: update.data,
                    indices: update.indices,
                    current: update.current,
                    end: update.end,
                    array: update.array,
                }
            }
        }

        impl<$($type: IntoAbstract),+> Shiperator for $non_packed<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            fn first_pass(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index = unsafe {std::ptr::read(self.indices.add(self.current))};
                    self.current += 1;
                    let data_indices = ($(
                        if $index == self.array {
                            self.current - 1
                        } else {
                            if let Some(index) = self.data.$index.index_of(index) {
                                index
                            } else {
                                continue
                            }
                        },
                    )+);
                    return Some(unsafe {($(self.data.$index.get_data(data_indices.$index),)+)})
                }
                None
            }
            fn post_process(&mut self, item: Self::Item) -> Self::Item {
                item
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(self.end - self.current))
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $non_packed<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                std::ptr::read(self.indices.add(self.current - 1))
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> UnindexedProducer for $non_packed<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn split(mut self) -> (Self, Option<Self>) {
                let len = self.end - self.current;
                if len >= 2 {
                    let clone = $non_packed {
                        data: ($(self.data.$index.clone(),)+),
                        indices: self.indices,
                        current: self.current + (len / 2),
                        end: self.end,
                        array: self.array,
                    };
                    self.end = clone.current;
                    (self, Some(clone))
                } else {
                    (self, None)
                }
            }
            fn fold_with<Fold>(self, folder: Fold) -> Fold where Fold: Folder<Self::Item> {
                folder.consume_iter(<Self as Shiperator>::into_iter(self))
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $non_packed1 $update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($update)* $update1; $($queue_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($non_packed: ident)*; $non_packed1: ident;
        $($update: ident)*; $update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $non_packed1 $update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
