use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;

macro_rules! impl_iterators {
    (
        $number: literal
        $non_packed: ident
        $(($type: ident, $index: tt, $index_type: ident))+
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

        impl<$($type: IntoAbstract),+> Shiperator for $non_packed<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            unsafe fn first_pass(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index = std::ptr::read(self.indices.add(self.current));
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
                    return Some(($(self.data.$index.get_data(data_indices.$index),)+))
                }
                None
            }
            unsafe fn post_process(&mut self, item: Self::Item) -> Self::Item {
                item
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $non_packed<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                std::ptr::read(self.indices.add(self.current - 1))
            }
        }

    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $(($type: ident, $index: tt, $index_type: ident))*;($type1: ident, $index1: tt, $index_type1: ident) $(($queue_type: ident, $queue_index: tt, $queue_index_type: ident))*
    ) => {
        impl_iterators![$number1 $non_packed1 $(($type, $index, $index_type))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $(($type, $index, $index_type))* ($type1, $index1, $index_type1); $(($queue_type, $queue_index, $queue_index_type))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($non_packed: ident)*; $non_packed1: ident;
        $(($type: ident, $index: tt, $index_type: ident))*;
    ) => {
        impl_iterators![$number1 $non_packed1 $(($type, $index, $index_type))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    (A, 0, usize) (B, 1, usize); (C, 2, usize) (D, 3, usize) (E, 4, usize) (F, 5, usize) (G, 6, usize) (H, 7, usize) (I, 8, usize) (J, 9, usize)
];
