use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Loose packed iterator over"]
        #[doc = $number]
        #[doc = "components.\n"]
        pub struct $loose<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) indices: *const EntityId,
            pub(crate) current: usize,
            pub(crate) end: usize,
            pub(crate) array: u32,
        }

        impl<$($type: IntoAbstract),+> Shiperator for $loose<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            unsafe fn first_pass(&mut self) -> Option<Self::Item> {
                if self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index = std::ptr::read(self.indices.add(self.current));
                    self.current += 1;
                    let indices = ($(
                        if (self.array >> $index) & 1 != 0 {
                            self.current - 1
                        } else {
                            self.data.$index.index_of_unchecked(index)
                        },
                    )+);
                    Some(($({
                        self.data.$index.get_data(indices.$index)
                    },)+))
                } else {
                    None
                }
            }
            unsafe fn post_process(&mut self, item: Self::Item) -> Self::Item {
                item
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $loose<$($type),+> {
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
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $loose1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($loose)* $loose1; $($queue_loose)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($loose: ident)*; $loose1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $loose1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
