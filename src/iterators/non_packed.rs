use super::{AbstractMut, IntoAbstract, IntoIter};

macro_rules! impl_non_packed {
    ($name: ident $(($type: ident, $index: tt))+) => {
        pub struct $name<$($type: IntoAbstract),+> {
            data: ($($type::View,)+),
            indices: *const usize,
            current: usize,
            end: usize,
            array: usize,
        }

        impl<$($type: IntoAbstract),+> IntoIter for ($($type,)+) {
            type IntoIter = $name<$($type,)+>;
            fn into_iter(self) -> Self::IntoIter {
                let mut smallest_index = std::usize::MAX;
                let mut i = 0;
                let mut tuple: (*const usize, usize) = (&0, std::usize::MAX);
                    $({
                        let new_tuple = self.$index.indices();
                        if let Some(new_len) = new_tuple.1 {
                            if new_len < tuple.1 {
                                smallest_index = i;
                                tuple = (new_tuple.0, new_len);
                            }
                        }
                        i += 1;
                    })+
                let _ = i;

                // if the user is trying to iterate over Not containers only
                if tuple.1 == std::usize::MAX {
                    tuple.1 = 0;
                }

                $name {
                    data: ($(self.$index.into_abstract(),)+),
                    indices: tuple.0,
                    current: 0,
                    end: tuple.1,
                    array: smallest_index,
                }
            }
        }
        impl<$($type: IntoAbstract),+> Iterator for $name<$($type,)+> {
            type Item = ($(<$type::View as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and self.indices can't access out of bounds
                    let index: usize = unsafe { std::ptr::read(self.indices.add(self.current)) };
                    self.current += 1;
                    return Some(($({
                        if $index == self.array {
                            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                            unsafe { self.data.$index.get_data(self.current - 1) }
                        } else {
                            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                            if let Some(item) = unsafe { self.data.$index.abs_get(index) } {
                                item
                            } else {
                                continue
                            }
                        }
                    },)+))
                }
                None
            }
        }
    }
}

macro_rules! non_packed {
    ($($left_name: ident)*; $name1: ident $($name: ident)+; $(($left_type: ident, $left_index: tt))*;($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_non_packed![$name1 $(($left_type, $left_index))*];
        non_packed![$($left_name)* $name1; $($name)+; $(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($($left_name: ident)*; $name: ident; $(($type: ident, $index: tt))*;) => {
        impl_non_packed![$name $(($type, $index))*];
    }
}

// There should be as many NonPacked as tuples
non_packed![;NonPacked2 NonPacked3 NonPacked4 NonPacked5; (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
