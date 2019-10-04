#[cfg(feature = "parallel")]
use super::m_update::*;
#[cfg(feature = "parallel")]
use super::{AbstractMut, IntoAbstract, ParBuf};
#[cfg(feature = "parallel")]
use crate::entity::Key;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::{bridge_unindexed, Folder, UnindexedConsumer, UnindexedProducer};
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

macro_rules! impl_iterators {
    (
        $number: literal
        $update: ident
        $par_update: ident
        $inner_par_update: ident
        $par_seq_update: ident
        $(($type: ident, $index: tt))+
    ) => {
        #[cfg(feature = "parallel")]
        pub struct $par_update<$($type: IntoAbstract),+>(pub(super) $update<$($type),+>);

        #[cfg(feature = "parallel")]
        impl<$($type: IntoAbstract),+> ParallelIterator for $par_update<$($type),+>
        where
            $(<$type::AbsView as AbstractMut>::Out: Send),+
        {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn drive_unindexed<Cons>(self, consumer: Cons) -> Cons::Result
            where
                Cons: UnindexedConsumer<Self::Item>,
            {
                use std::sync::atomic::Ordering;

                let mut data = self.0.data.clone();
                let len = self.0.end - self.0.current;
                let updated = ParBuf::new(len);

                let inner = $inner_par_update {
                    iter: self.0,
                    updated: &updated,
                };

                let result = bridge_unindexed(inner, consumer);
                let slice = unsafe {
                    std::slice::from_raw_parts(updated.buf, updated.len.load(Ordering::Relaxed))
                };
                for &index in slice {
                    $(
                        unsafe {data.$index.mark_id_modified(index)};
                    )+
                }
                result
            }
        }

        #[cfg(feature = "parallel")]
        pub struct $inner_par_update<'a, $($type: IntoAbstract),+> {
            iter: $update<$($type),+>,
            updated: &'a ParBuf<Key>,
        }

        #[cfg(feature = "parallel")]
        impl<'a, $($type: IntoAbstract),+> $inner_par_update<'a, $($type),+> {
            fn clone(&self) -> Self {
                let iter = $update {
                    data: self.iter.data.clone(),
                    indices: self.iter.indices,
                    current: self.iter.current,
                    end: self.iter.end,
                    array: self.iter.array,
                    last_id: Key::dead(),
                };

                $inner_par_update {
                    iter,
                    updated: self.updated,
                }
            }
        }

        #[cfg(feature = "parallel")]
        impl<'a, $($type: IntoAbstract),+> UnindexedProducer for $inner_par_update<'a, $($type,)+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn split(mut self) -> (Self, Option<Self>) {
                let len = self.iter.end - self.iter.current;
                if len >= 2 {
                    let mut clone = self.clone();
                    clone.iter.current += len / 2;
                    self.iter.end = clone.iter.current;
                    (self, Some(clone))
                } else {
                    (self, None)
                }
            }
            fn fold_with<Fold>(self, folder: Fold) -> Fold
            where Fold: Folder<Self::Item> {
                let iter: $par_seq_update<$($type),+> = $par_seq_update {
                    updated: self.updated,
                    data: self.iter.data,
                    indices: self.iter.indices,
                    current: self.iter.current,
                    end: self.iter.end,
                    array: self.iter.array,
                };
                folder.consume_iter(iter)
            }
        }

        #[cfg(feature = "parallel")]
        pub struct $par_seq_update<'a, $($type: IntoAbstract),+> {
            data: ($($type::AbsView,)+),
            current: usize,
            end: usize,
            indices: *const Key,
            updated: &'a ParBuf<Key>,
            array: usize,
        }

        #[cfg(feature = "parallel")]
        impl<'a, $($type: IntoAbstract),+> Iterator for $par_seq_update<'a, $($type),+> {
            type Item = ($(<$type::AbsView as AbstractMut>::Out,)+);
            fn next(&mut self) -> Option<Self::Item> {
                while self.current < self.end {
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

                    self.updated.push(index);

                    unsafe {
                        return Some(($(self.data.$index.get_data(data_indices.$index),)+))
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
        $($par_update: ident)*; $par_update1: ident $($queue_par_update: ident)+;
        $($inner_par_update: ident)*; $inner_par_update1: ident $($queue_inner_par_update: ident)+;
        $($par_seq_update: ident)*; $par_seq_update1: ident $($queue_par_seq_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$number1 $update1 $par_update1 $inner_par_update1 $par_seq_update1 $(($type, $index))*];
        iterators![
            $($number)* $number1; $($queue_number)+;
            $($update)* $update1; $($queue_update)+;
            $($par_update)* $par_update1; $($queue_par_update)+;
            $($inner_par_update)* $inner_par_update1; $($queue_inner_par_update)+;
            $($par_seq_update)* $par_seq_update1; $($queue_par_seq_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($number: literal)*; $number1: literal;
        $($update: ident)*; $update1: ident;
        $($par_update: ident)*; $par_update1: ident;
        $($inner_par_update: ident)*; $inner_par_update1: ident;
        $($par_seq_update: ident)*; $par_seq_update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$number1 $update1 $par_update1 $inner_par_update1 $par_seq_update1 $(($type, $index))*];
    }
}

iterators![
    ;"2" "3" "4" "5" "6" "7" "8" "9" "10";
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    ;ParUpdate2 ParUpdate3 ParUpdate4 ParUpdate5 ParUpdate6 ParUpdate7 ParUpdate8 ParUpdate9 ParUpdate10;
    ;InnerParUpdate2 InnerParUpdate3 InnerParUpdate4 InnerParUpdate5 InnerParUpdate6 InnerParUpdate7 InnerParUpdate8 InnerParUpdate9 InnerParUpdate10;
    ;ParSeqUpdate2 ParSeqUpdate3 ParSeqUpdate4 ParSeqUpdate5 ParSeqUpdate6 ParSeqUpdate7 ParSeqUpdate8 ParSeqUpdate9 ParSeqUpdate10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
