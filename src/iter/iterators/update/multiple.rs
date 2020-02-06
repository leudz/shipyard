use super::{AbstractMut, CurrentId, IntoAbstract, Shiperator};
use crate::EntityId;
use core::ptr;

macro_rules! impl_iterators {
    (
        $number: literal
        $update: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $update<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) indices: *const EntityId,
            pub(crate) current: usize,
            pub(crate) end: usize,
            pub(crate) array: usize,
            pub(crate) current_id: EntityId,
        }

        impl<$($type: IntoAbstract),+> Shiperator for $update<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            fn first_pass(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index = unsafe {ptr::read(self.indices.add(self.current))};
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
                    self.current_id = index;
                    return Some(unsafe {($(self.data.$index.get_data(data_indices.$index),)+)})
                }
                None
            }
            fn post_process(&mut self, _: Self::Item) -> Self::Item {
                unsafe {($(self.data.$index.mark_id_modified(self.current_id),)+)}
            }
            fn size_hint(&self) -> (usize, Option<usize>) {
                (0, Some(self.end - self.current))
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $update<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                self.current_id
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($update)* $update1; $($queue_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($update: ident)*; $update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
