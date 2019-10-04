use super::{AbstractMut, IntoAbstract};
use crate::entity::Key;

macro_rules! impl_iterators {
    (
        $number: literal
        $update_filter: ident
        $(($type: ident, $index: tt))+
    ) => {
        pub struct $update_filter<$($type: IntoAbstract),+, P> {
            pub(super) data: ($($type::AbsView,)+),
            pub(super) indices: *const Key,
            pub(super) current: usize,
            pub(super) end: usize,
            pub(super) array: usize,
            pub(super) last_id: Key,
            pub(super) pred: P,
        }

        impl<$($type: IntoAbstract),+, P: FnMut(&($(<<$type as IntoAbstract>::AbsView as AbstractMut>::Out),+)) -> bool> Iterator for $update_filter<$($type),+, P> {
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
                    unsafe {
                        if (self.pred)(&($(self.data.$index.get_data(data_indices.$index),)+)) {
                            self.last_id = self.data.0.id_at(self.current - 1);
                            return Some(($(self.data.$index.mark_modified(data_indices.$index),)+))
                        }
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
        $($update_filter: ident)*; $update_filter1: ident $($queue_update_filter: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $update_filter1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($update_filter)* $update_filter1; $($queue_update_filter)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($update_filter: ident)*; $update_filter1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $update_filter1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;UpdateFilter2 UpdateFilter3 UpdateFilter4 UpdateFilter5 UpdateFilter6 UpdateFilter7 UpdateFilter8 UpdateFilter9 UpdateFilter10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
