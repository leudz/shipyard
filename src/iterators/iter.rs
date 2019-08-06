use super::{AbstractMut, IntoAbstract, IntoIter};
use std::marker::PhantomData;

// Packed iterators go from start to end without index lookup
// They only work in specific circumstances but are the fastest
pub struct Packed1<T: IntoAbstract> {
    data: T::View,
    current: usize,
    end: usize,
}

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Packed1<Self>;
    fn into_iter(self) -> Self::IntoIter {
        Packed1 {
            end: self.indices().1.unwrap_or(0),
            data: self.into_abstract(),
            current: 0,
        }
    }
}

impl<T: IntoAbstract> Iterator for Packed1<T> {
    type Item = <T::View as AbstractMut>::Out;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        if current < self.end {
            self.current += 1;
            // SAFE the index is valid and the iterator can only be created where the lifetime is valid
            Some(unsafe { self.data.get_data(current) })
        } else {
            None
        }
    }
}

macro_rules! impl_iterators {
    ($iter: ident $packed: ident $non_packed: ident $(($type: ident, $index: tt))+) => {
        pub struct $packed<$($type: IntoAbstract),+> {
            data: ($($type::View,)+),
            current: usize,
            end: usize,
        }

        pub struct $non_packed<$($type: IntoAbstract),+> {
            data: ($($type::View,)+),
            indices: *const usize,
            current: usize,
            end: usize,
            array: usize,
        }

        pub enum $iter<$($type: IntoAbstract),+> {
            Packed($packed<$($type),+>),
            NonPacked($non_packed<$($type),+>),
        }

        impl<$($type: IntoAbstract),+> Iterator for $iter<$($type),+> {
            type Item = ($(<$type::View as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $iter::Packed(iter) => iter.next(),
                    $iter::NonPacked(iter) => iter.next(),
                }
            }
        }

        impl<$($type: IntoAbstract),+> IntoIter for ($($type,)+) {
            type IntoIter = $iter<$($type,)+>;
            fn into_iter(self) -> Self::IntoIter {
                // check if all types are packed together
                let packed_types = self.0.abs_pack_types_owned();
                if $(std::ptr::eq(packed_types, self.$index.abs_pack_types_owned()))&&+ && {
                    let mut i = 0;
                    $(let _: PhantomData<$type> = {i += 1; PhantomData};)+
                    i == packed_types.len()
                } {
                    $iter::Packed($packed {
                        end: self.0.abs_pack_len(),
                        data: ($(self.$index.into_abstract(),)+),
                        current: 0,
                    })
                } else {
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

                    $iter::NonPacked($non_packed {
                        data: ($(self.$index.into_abstract(),)+),
                        indices: tuple.0,
                        current: 0,
                        end: tuple.1,
                        array: smallest_index,
                    })
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $packed<$($type),+> {
            type Item = ($(<$type::View as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current < self.end {
                    self.current += 1;
                    // SAFE the index is valid and the iterator can only be created where the lifetime is valid
                    Some(unsafe { ($(self.data.$index.get_data(current),)+) })
                } else {
                    None
                }
            }
        }

        impl<$($type: IntoAbstract),+> Iterator for $non_packed<$($type,)+> {
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

macro_rules! iterators {
    (
        $($iter: ident)*; $iter1: ident $($queue_iter: ident)+;
        $($packed: ident)*; $packed1: ident $($queue_packed: ident)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$iter1 $packed1 $non_packed1 $(($type, $index))*];
        iterators![
            $($iter)* $iter1; $($queue_iter)+;
            $($packed)* $packed1; $($queue_packed)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($iter: ident)*; $iter1: ident;
        $($packed: ident)*; $packed1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$iter1 $packed1 $non_packed1 $(($type, $index))*];
    }
}

iterators![;Iter2 Iter3 Iter4 Iter5;; Packed2 Packed3 Packed4 Packed5;; NonPacked2 NonPacked3 NonPacked4 NonPacked5; (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
