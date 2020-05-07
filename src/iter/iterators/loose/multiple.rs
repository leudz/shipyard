use super::{
    AbstractMut, CurrentId, DoubleEndedShiperator, ExactSizeShiperator, IntoAbstract, IntoIterator,
    Shiperator,
};
use crate::EntityId;
use core::ptr;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

macro_rules! impl_iterators {
    (
        $number: literal
        $loose: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[doc = "Loose iterator over"]
        #[doc = $number]
        #[doc = "components."]
        pub struct $loose<$($type: IntoAbstract),+> {
            pub(crate) data: ($($type::AbsView,)+),
            pub(crate) indices: *const EntityId,
            pub(crate) current: usize,
            pub(crate) end: usize,
            pub(crate) array: u32,
        }

        unsafe impl<$($type: IntoAbstract),+> Send for $loose<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {}

        impl<$($type: IntoAbstract),+> Clone for $loose<$($type),+> where $($type::AbsView: Clone + Copy),+ {
            fn clone(&self) -> Self {
                $loose {
                    data: self.data,
                    indices: self.indices,
                    current: self.current,
                    end: self.end,
                    array: self.array,
                }
            }
        }

        impl<$($type: IntoAbstract),+> Copy for $loose<$($type),+> where $($type::AbsView: Clone + Copy),+ {}

        impl<$($type: IntoAbstract),+> Shiperator for $loose<$($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);

            fn first_pass(&mut self) -> Option<Self::Item> {
                let current = self.current;
                if current < self.end {
                    self.current += 1;
                    // SAFE at this point there are no mutable reference to sparse or dense
                    // and current is in bound
                    let index = unsafe {ptr::read(self.indices.add(current))};
                    let indices = ($(
                        if (self.array >> $index) & 1 != 0 {
                            current
                        } else {
                            // SAFE the entity has a component in this storage
                            unsafe {self.data.$index.index_of_unchecked(index)}
                        },
                    )+);
                    Some(unsafe {($({
                        self.data.$index.get_data(indices.$index)
                    },)+)})
                } else {
                    None
                }
            }
            fn post_process(&mut self) {}
            fn size_hint(&self) -> (usize, Option<usize>) {
                let len = self.end - self.current;
                (len, Some(len))
            }
        }

        impl<$($type: IntoAbstract),+> CurrentId for $loose<$($type),+> {
            type Id = EntityId;

            unsafe fn current_id(&self) -> Self::Id {
                ptr::read(self.indices.add(self.current - 1))
            }
        }

        impl<$($type: IntoAbstract),+> ExactSizeShiperator for $loose<$($type),+> {}

        impl<$($type: IntoAbstract),+> DoubleEndedShiperator for $loose<$($type),+> {
            fn first_pass_back(&mut self) -> Option<Self::Item> {
                if self.current < self.end {
                    self.end -= 1;
                    // SAFE we checked for OOB
                    let index = unsafe {ptr::read(self.indices.add(self.current))};
                    let indices = ($(
                        if (self.array >> $index) & 1 != 0 {
                            self.end
                        } else {
                            // SAFE the entity has a component in this storage
                            unsafe {self.data.$index.index_of_unchecked(index)}
                        },
                    )+);
                    Some(unsafe {($({
                        self.data.$index.get_data(indices.$index)
                    },)+)})
                } else {
                    None
                }
            }
        }

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> Producer for $loose<$($type),+>
        where $($type::AbsView: Clone + Send,)+ $(<$type::AbsView as AbstractMut>::Out: Send),+ {
            type Item = ($(<$type::AbsView as AbstractMut>::Out),+);
            type IntoIter = IntoIterator<Self>;
            fn into_iter(self) -> Self::IntoIter {
                core::iter::IntoIterator::into_iter(self)
            }
            fn split_at(mut self, index: usize) -> (Self, Self) {
                let clone = $loose {
                    data: ($(self.data.$index.clone(),)+),
                    indices: self.indices,
                    current: self.current + index,
                    end: self.end,
                    array: self.array,
                };
                self.end = clone.current;
                (self, clone)
            }
        }

        impl<$($type: IntoAbstract),+> core::iter::IntoIterator for $loose<$($type),+> {
            type IntoIter = IntoIterator<Self>;
            type Item = <Self as Shiperator>::Item;
            fn into_iter(self) -> Self::IntoIter {
                IntoIterator(self)
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
