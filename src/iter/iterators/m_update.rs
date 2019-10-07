use super::m_update_filter::*;
use super::m_update_with_id::*;
use super::{AbstractMut, IntoAbstract};
use crate::entity::Key;

macro_rules! impl_iterators {
    (
        $number: literal
        $update: ident
        $update_filter: ident
        $update_with_id: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $update<$($type: IntoAbstract),+> {
            pub(super) data: ($($type::AbsView,)+),
            pub(super) indices: *const Key,
            pub(super) current: usize,
            pub(super) end: usize,
            pub(super) array: usize,
            pub(super) last_id: Key,
        }

        impl<$($type: IntoAbstract),+> $update<$($type),+> {
            pub fn filtered<P: FnMut(&<Self as Iterator>::Item) -> bool>(self, pred: P) -> $update_filter<$($type),+, P> {
                $update_filter {
                    data: self.data,
                    indices: self.indices,
                    current: self.current,
                    end: self.end,
                    array: self.array,
                    last_id: Key::dead(),
                    pred,
                }
            }
            pub fn with_id(self) -> $update_with_id<$($type),+> {
                $update_with_id(self)
            }
        }

        unsafe impl<$($type: IntoAbstract),+> Send for $update<$($type),+> {}

        impl<$($type: IntoAbstract,)+> Iterator for $update<$($type),+> {
            type Item = ($(<<$type as IntoAbstract>::AbsView as AbstractMut>::Out),+);
            fn next(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index = unsafe { std::ptr::read(self.indices.add(self.current)) };
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
                    self.last_id = index;
                    unsafe {
                        return Some(($(self.data.$index.mark_modified(data_indices.$index),)+))
                    }
                }
                None
            }
        }
    }
}

macro_rules! iterators {
    (
        $($number: literal)*; $number1: literal $($queue_number: literal)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $($update_filter: ident)*; $update_filter1: ident $($queue_update_filter: ident)+;
        $($update_with_id: ident)*; $update_with_id1: ident $($queue_update_with_id: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $update1 $update_filter1 $update_with_id1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($update)* $update1; $($queue_update)+;
            $($update_filter)* $update_filter1; $($queue_update_filter)+;
            $($update_with_id)* $update_with_id1; $($queue_update_with_id)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($update: ident)*; $update1: ident;
        $($update_filter: ident)*; $update_filter1: ident;
        $($update_with_id: ident)*; $update_with_id1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $update1 $update_filter1 $update_with_id1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    ;UpdateFilter2 UpdateFilter3 UpdateFilter4 UpdateFilter5 UpdateFilter6 UpdateFilter7 UpdateFilter8 UpdateFilter9 UpdateFilter10;
    ;UpdateWithId2 UpdateWithId3 UpdateWithId4 UpdateWithId5 UpdateWithId6 UpdateWithId7 UpdateWithId8 UpdateWithId9 UpdateWithId10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
