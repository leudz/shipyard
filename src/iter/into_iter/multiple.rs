use super::*;
use crate::sparse_set::Pack;
use crate::EntityId;
use core::ptr;

macro_rules! impl_iterators {
    (
        $iter: ident
        $par_iter: ident
        $tight: ident
        $loose: ident
        $non_packed: ident
        $update: ident
        $(($type: ident, $index: tt))+
    ) => {
        impl<$($type: IntoAbstract),+> IntoIter for ($($type,)+) {
            type IntoIter = $iter<$($type,)+>;
            #[cfg(feature = "parallel")]
            type IntoParIter = $par_iter<$($type,)+>;
            fn iter(self) -> Self::IntoIter {
                #[derive(PartialEq, Eq)]
                enum PackIter {
                    Tight,
                    Loose,
                    Update,
                    None,
                }

                let mut type_ids = [$(self.$index.type_id()),+];
                type_ids.sort_unstable();
                let mut smallest_index = core::usize::MAX;
                let mut smallest = core::usize::MAX;
                let mut i = 0;
                let mut pack_iter = PackIter::None;
                let is_offseted = $(self.$index.offset() != 0)||+;

                $({
                    if pack_iter == PackIter::None || pack_iter == PackIter::Update {
                        match (&self.$index.pack_info().pack, is_offseted) {
                            (Pack::Tight(pack), false) => {
                                if let Ok(types) = pack.check_types(&type_ids) {
                                    if types.len() == type_ids.len() {
                                        pack_iter = PackIter::Tight;
                                        smallest = pack.len;
                                    } else if pack.len < smallest {
                                        smallest = pack.len;
                                        smallest_index = i;
                                    }
                                } else if let Some(len) = self.$index.len() {
                                    if len < smallest {
                                        smallest = len;
                                        smallest_index = i;
                                    }
                                }
                            }
                            (Pack::Loose(pack), false) => {
                                if pack.check_all_types(&type_ids).is_ok() {
                                    if pack.tight_types.len() + pack.loose_types.len() == type_ids.len() {
                                        pack_iter = PackIter::Loose;
                                        smallest = pack.len;
                                        smallest_index = i;
                                    } else if pack.len < smallest {
                                        smallest = pack.len;
                                        smallest_index = i;
                                    }
                                } else if let Some(len) = self.$index.len() {
                                    if len < smallest {
                                        smallest = len;
                                        smallest_index = i;
                                    }
                                }
                            }
                            (Pack::Update(_), _) => {
                                pack_iter = PackIter::Update;
                                if let Some(len) = self.$index.len() {
                                    if len < smallest {
                                        smallest = len;
                                        smallest_index = i;
                                    }
                                }
                            }
                            _ => if let Some(len) = self.$index.len() {
                                if len < smallest {
                                    smallest = len;
                                    smallest_index = i;
                                }
                            }
                        }

                        i += 1;
                    }
                })+

                let _ = i;

                match pack_iter {
                    PackIter::Tight => {
                        $(
                            let smallest = self.$index.len().unwrap_or(0).min(smallest);
                        )+
                        $iter::Tight($tight {
                            data: ($(self.$index.into_abstract(),)+),
                            current: 0,
                            end: smallest,
                        })
                    }
                    PackIter::Loose => {
                        $(
                            let smallest = self.$index.len().unwrap_or(0).min(smallest);
                        )+
                        let mut indices = None;
                        let mut array = 0;
                        $(
                            if let Pack::Loose(_) = self.$index.pack_info().pack {
                                array |= 1 << $index;
                            }
                        )+
                        let data = ($(self.$index.into_abstract(),)+);
                        $(
                            if $index == smallest_index {
                                indices = Some(data.$index.dense());
                            }
                        )+
                        $iter::Loose($loose {
                            data,
                            current: 0,
                            end: smallest,
                            array,
                            indices: indices.unwrap(),
                        })
                    }
                    PackIter::Update => {
                        let mut indices = None;
                        let data = ($(self.$index.into_abstract(),)+);
                        // if the user is trying to iterate over Not containers only
                        if smallest == core::usize::MAX {
                            smallest = 0;
                        } else {
                            $(
                                if $index == smallest_index {
                                    indices = Some(data.$index.dense());
                                }
                            )+
                        }

                        $iter::Update($update {
                            data,
                            indices: indices.unwrap_or(ptr::null()),
                            current: 0,
                            end: smallest,
                            array: smallest_index,
                            current_id: EntityId::dead(),
                        })
                    }
                    PackIter::None => {
                        let mut indices = ptr::null();
                        let data = ($(self.$index.into_abstract(),)+);
                        // if the user is trying to iterate over Not containers only
                        if smallest == core::usize::MAX {
                            smallest = 0;
                        } else {
                            $(
                                if $index == smallest_index {
                                    indices = data.$index.dense();
                                }
                            )+
                        }

                        $iter::NonPacked($non_packed {
                            data,
                            indices,
                            current: 0,
                            end: smallest,
                            array: smallest_index,
                        })
                    }
                }
            }
            #[cfg(feature = "parallel")]
            fn par_iter(self) -> Self::IntoParIter {
                match self.iter() {
                    $iter::Tight(tight) => $par_iter::Tight(tight.into()),
                    $iter::Loose(loose) => $par_iter::Loose(loose.into()),
                    $iter::Update(update) => $par_iter::NonPacked(update.into()),
                    $iter::NonPacked(non_packed) => $par_iter::NonPacked(non_packed.into()),
                }
            }
        }
    }
}

macro_rules! iterators {
    (
        $($iter: ident)*; $iter1: ident $($queue_iter: ident)+;
        $($par_iter: ident)*; $par_iter1: ident $($queue_par_iter: ident)+;
        $($tight: ident)*; $tight1: ident $($queue_tight: ident)+;
        $($loose: ident)*; $loose1: ident $($queue_loose: ident)+;
        $($non_packed: ident)*; $non_packed1: ident $($queue_non_packed: ident)+;
        $($update: ident)*; $update1: ident $($queue_update: ident)+;
        $(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*
    ) => {
        impl_iterators![$iter1 $par_iter1 $tight1 $loose1 $non_packed1 $update1 $(($type, $index))*];
        iterators![
            $($iter)* $iter1; $($queue_iter)+;
            $($par_iter)* $par_iter1; $($queue_par_iter)+;
            $($tight)* $tight1; $($queue_tight)+;
            $($loose)* $loose1; $($queue_loose)+;
            $($non_packed)* $non_packed1; $($queue_non_packed)+;
            $($update)* $update1; $($queue_update)+;
            $(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*
        ];
    };
    (
        $($iter: ident)*; $iter1: ident;
        $($par_iter: ident)*; $par_iter1: ident;
        $($tight: ident)*; $tight1: ident;
        $($loose: ident)*; $loose1: ident;
        $($non_packed: ident)*; $non_packed1: ident;
        $($update: ident)*; $update1: ident;
        $(($type: ident, $index: tt))*;
    ) => {
        impl_iterators![$iter1 $par_iter1 $tight1 $loose1 $non_packed1 $update1 $(($type, $index))*];
    }
}

iterators![
    ;Iter2 Iter3 Iter4 Iter5 Iter6 Iter7 Iter8 Iter9 Iter10;
    ;ParIter2 ParIter3 ParIter4 ParIter5 ParIter6 ParIter7 ParIter8 ParIter9 ParIter10;
    ;Tight2 Tight3 Tight4 Tight5 Tight6 Tight7 Tight8 Tight9 Tight10;
    ;Loose2 Loose3 Loose4 Loose5 Loose6 Loose7 Loose8 Loose9 Loose10;
    ;NonPacked2 NonPacked3 NonPacked4 NonPacked5 NonPacked6 NonPacked7 NonPacked8 NonPacked9 NonPacked10;
    ;Update2 Update3 Update4 Update5 Update6 Update7 Update8 Update9 Update10;
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
];
