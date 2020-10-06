use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iter::Iter;
use super::loose::Loose;
use super::mixed::Mixed;
#[cfg(feature = "parallel")]
use super::par_iter::ParIter;
use super::tight::Tight;
use crate::sparse_set::Pack;
use crate::storage::EntityId;
use crate::type_id::TypeId;
use core::ptr;

const ACCESS_FACTOR: usize = 3;

pub trait IntoIter {
    type IntoIter;
    #[cfg(feature = "parallel")]
    type IntoParIter;

    fn iter(self) -> Self::IntoIter;
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter;
}

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Iter<T::AbsView>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter<T::AbsView>;

    #[inline]
    fn iter(self) -> Self::IntoIter {
        match self.len() {
            Some((len, true)) => Iter::Tight(Tight {
                current: 0,
                end: len,
                storage: self.into_abstract(),
            }),
            Some((len, false)) => Iter::Mixed(Mixed {
                indices: self.dense(),
                storage: self.into_abstract(),
                current: 0,
                end: len,
                mask: 0,
                last_id: EntityId::dead(),
            }),
            None => Iter::Tight(Tight {
                current: 0,
                end: 0,
                storage: self.into_abstract(),
            }),
        }
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn par_iter(self) -> Self::IntoParIter {
        self.iter().into()
    }
}

impl<T: IntoAbstract> IntoIter for (T,)
where
    <T::AbsView as AbstractMut>::Index: From<usize>,
{
    type IntoIter = Iter<(T::AbsView,)>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter<(T::AbsView,)>;

    #[inline]
    fn iter(self) -> Self::IntoIter {
        match self.0.len() {
            Some((len, true)) => Iter::Tight(Tight {
                current: 0,
                end: len,
                storage: (self.0.into_abstract(),),
            }),
            Some((len, false)) => Iter::Mixed(Mixed {
                current: 0,
                end: len,
                indices: self.0.dense(),
                mask: 0,
                last_id: EntityId::dead(),
                storage: (self.0.into_abstract(),),
            }),
            None => Iter::Tight(Tight {
                current: 0,
                end: 0,
                storage: (self.0.into_abstract(),),
            }),
        }
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn par_iter(self) -> Self::IntoParIter {
        self.iter().into()
    }
}

macro_rules! impl_into_iter {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))+) => {
        impl<$type1: IntoAbstract, $($type: IntoAbstract),+> IntoIter for ($type1, $($type,)+) where <$type1::AbsView as AbstractMut>::Index: From<usize>, $(<$type::AbsView as AbstractMut>::Index: From<usize>),+ {
            type IntoIter = Iter<($type1::AbsView, $($type::AbsView,)+)>;
            #[cfg(feature = "parallel")]
            type IntoParIter = ParIter<($type1::AbsView, $($type::AbsView,)+)>;

            #[allow(clippy::drop_copy)]
            fn iter(self) -> Self::IntoIter {
                #[derive(PartialEq, Eq)]
                enum IterPack {
                    Tight,
                    Loose,
                    Mixed,
                }

                let mut type_ids = [self.$index1.type_id(), $(self.$index.type_id()),+];
                type_ids.sort_unstable();
                let mut iter_pack = IterPack::Mixed;
                let mut smallest = core::usize::MAX;
                let mut smallest_pack_info: *const [TypeId] = ptr::slice_from_raw_parts(ptr::null(), 0);
                let mut smallest_dense = ptr::null();
                let mut factored_len = core::usize::MAX;
                let mut mask: u16 = 0;
                let mut full_loose = true;

                match &self.$index1.metadata().pack {
                    Pack::Tight(tight) => {
                        if tight.is_packable(&type_ids) {
                            if tight.types.len() == type_ids.len() {
                                iter_pack = IterPack::Tight;
                                smallest = tight.len;
                                smallest_pack_info = &*tight.types;
                                smallest_dense = self.$index1.dense();
                                if let Some((_, true)) = self.$index1.len() {
                                    mask = 1 << $index1;
                                }
                            } else {
                                smallest = tight.len;
                                smallest_pack_info = &*tight.types;
                                smallest_dense = self.$index1.dense();
                                factored_len = smallest * tight.types.len() + smallest * (type_ids.len() - tight.types.len()) * ACCESS_FACTOR;
                                if let Some((_, true)) = self.$index1.len() {
                                    mask = 1 << $index1;
                                }
                            }
                        } else if let Some((len, is_exact)) = self.$index1.len() {
                            smallest = len;
                            smallest_dense = self.$index1.dense();
                            if is_exact {
                                factored_len = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;
                                mask = 1 << $index1;
                            } else {
                                factored_len = len * type_ids.len() * ACCESS_FACTOR;
                            }
                        }
                    }
                    Pack::Loose(loose) => {
                        if loose.is_packable(&type_ids) {
                            if loose.tight_types.len() + loose.loose_types.len() == type_ids.len() {
                                iter_pack = IterPack::Loose;
                                smallest = loose.len;
                                smallest_pack_info = &*loose.tight_types;
                                smallest_dense = self.$index1.dense();
                                if let Some((_, true)) = self.$index1.len() {
                                    mask = 1 << $index1;
                                } else {
                                    full_loose = false;
                                }
                            } else {
                                smallest = loose.len;
                                smallest_pack_info = &*loose.tight_types;
                                smallest_dense = self.$index1.dense();
                                factored_len = smallest * loose.tight_types.len() + smallest * (type_ids.len() - loose.tight_types.len()) * ACCESS_FACTOR;
                                if let Some((_, true)) = self.$index1.len() {
                                    mask = 1 << $index1;
                                }
                            }
                        } else if let Some((len, is_exact)) = self.$index1.len() {
                            smallest = len;
                            smallest_dense = self.$index1.dense();
                            if is_exact {
                                factored_len = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;
                                mask = 1 << $index1;
                            } else {
                                factored_len = len * type_ids.len() * ACCESS_FACTOR;
                            }
                        }
                    },
                    Pack::None => {
                        if let Some((len, is_exact)) = self.$index1.len() {
                            smallest = len;
                            smallest_dense = self.$index1.dense();
                            if is_exact {
                                factored_len = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;
                                mask = 1 << $index1;
                            } else {
                                factored_len = len * type_ids.len() * ACCESS_FACTOR;
                            }
                        }
                    }
                }

                $(
                    match &mut iter_pack {
                        IterPack::Tight => {
                            if let Some((_, true)) = self.$index.len() {
                                mask |= 1 << $index;
                            }
                        }
                        IterPack::Loose => {
                            if let Pack::Loose(_) = self.$index.metadata().pack {
                                if let Some((_, true)) = self.$index.len() {
                                    mask |= 1 << $index;
                                } else {
                                    full_loose = false;
                                }
                            }
                        },
                        IterPack::Mixed => {
                            match &self.$index.metadata().pack {
                                Pack::Tight(tight) => {
                                    let tight_types: *const _ = &*tight.types;

                                    if tight_types == smallest_pack_info {
                                        if let Some((_, true)) = self.$index.len() {
                                            mask |= 1 << $index;
                                        }
                                    } else {
                                        if tight.is_packable(&type_ids) {
                                            let factor = tight.len * tight.types.len() + tight.len * (type_ids.len() - tight.types.len()) * ACCESS_FACTOR;

                                            if factor < factored_len {
                                                smallest = tight.len;
                                                smallest_pack_info = &*tight.types;
                                                smallest_dense = self.$index.dense();
                                                factored_len = factor;
                                                if let Some((_, true)) = self.$index.len() {
                                                    mask = 1 << $index;
                                                }
                                            }
                                        } else if let Some((len, is_exact)) = self.$index.len() {
                                            if is_exact {
                                                let factor = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;

                                                if factor < factored_len {
                                                    smallest = len;
                                                    smallest_dense = self.$index.dense();
                                                    factored_len = factor;
                                                    mask = 1 << $index;
                                                }
                                            } else {
                                                let factor = len * type_ids.len() * ACCESS_FACTOR;

                                                if factor < factored_len {
                                                    smallest = len;
                                                    smallest_dense = self.$index.dense();
                                                    factored_len = factor;
                                                }
                                            }
                                        }
                                    }
                                }
                                Pack::Loose(loose) => {
                                    let loose_types: *const _ = &*loose.tight_types;

                                    if loose_types == smallest_pack_info {
                                        if let Some((_, true)) = self.$index.len() {
                                            mask |= 1 << $index;
                                        } else {
                                            full_loose = false;
                                        }
                                    } else {
                                        if loose.is_packable(&type_ids) {
                                            if loose.tight_types.len() + loose.loose_types.len() == type_ids.len() {
                                                iter_pack = IterPack::Loose;
                                                smallest = loose.len;
                                                smallest_pack_info = &*loose.tight_types;
                                                smallest_dense = self.$index.dense();
                                                if let Some((_, true)) = self.$index.len() {
                                                    mask = 1 << $index;
                                                } else {
                                                    full_loose = false;
                                                }
                                            } else {
                                                let factor = loose.len * loose.tight_types.len() + loose.len * (type_ids.len() - loose.tight_types.len()) * ACCESS_FACTOR;

                                                if factor < factored_len {
                                                    smallest = loose.len;
                                                    smallest_pack_info = &*loose.tight_types;
                                                    smallest_dense = self.$index.dense();
                                                    factored_len = factor;
                                                    if let Some((_, true)) = self.$index.len() {
                                                        mask = 1 << $index;
                                                    }
                                                }
                                            }
                                        } else if let Some((len, is_exact)) = self.$index.len() {
                                            if is_exact {
                                                let factor = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;

                                                if factor < factored_len {
                                                    smallest = len;
                                                    smallest_dense = self.$index.dense();
                                                    factored_len = factor;
                                                    mask = 1 << $index;
                                                }
                                            } else {
                                                let factor = len * type_ids.len() * ACCESS_FACTOR;

                                                if factor < factored_len {
                                                    smallest = len;
                                                    smallest_dense = self.$index.dense();
                                                    factored_len = factor;
                                                }
                                            }
                                        }
                                    }
                                },
                                Pack::None => {
                                    if let Some((len, is_exact)) = self.$index.len() {
                                        if is_exact {
                                            let factor = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;

                                            if factor < factored_len {
                                                smallest = len;
                                                smallest_dense = self.$index.dense();
                                                factored_len = factor;
                                                mask = 1 << $index;
                                            }
                                        } else {
                                            let factor = len * type_ids.len() * ACCESS_FACTOR;

                                            if factor < factored_len {
                                                smallest = len;
                                                smallest_dense = self.$index.dense();
                                                factored_len = factor;
                                            }
                                        }
                                    }
                                }
                            }
                        },
                    }
                )+

                drop(smallest_pack_info);
                drop(factored_len);

                match iter_pack {
                    IterPack::Tight => {
                        if mask.count_ones() == type_ids.len() as u32 {
                            Iter::Tight(Tight {
                                current: 0,
                                end: smallest,
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        } else {
                            Iter::Mixed(Mixed {
                                current: 0,
                                end: smallest,
                                mask,
                                indices: smallest_dense,
                                last_id: EntityId::dead(),
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        }
                    }
                    IterPack::Loose => {
                        if full_loose {
                            Iter::Loose(Loose {
                                current: 0,
                                end: smallest,
                                indices: smallest_dense,
                                mask,
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        } else {
                            Iter::Mixed(Mixed {
                                current: 0,
                                end: smallest,
                                mask,
                                indices: smallest_dense,
                                last_id: EntityId::dead(),
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        }
                    }
                    IterPack::Mixed => {
                        if smallest == core::usize::MAX {
                            Iter::Mixed(Mixed {
                                current: 0,
                                end: 0,
                                mask,
                                indices: smallest_dense,
                                last_id: EntityId::dead(),
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        } else {
                            Iter::Mixed(Mixed {
                                current: 0,
                                end: smallest,
                                mask,
                                indices: smallest_dense,
                                last_id: EntityId::dead(),
                                storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            })
                        }
                    }
                }
            }
            #[cfg(feature = "parallel")]
            #[inline]
            fn par_iter(self) -> Self::IntoParIter {
                self.iter().into()
            }
        }
    }
}

macro_rules! into_iter {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_iter![$(($type, $index))+];
        into_iter![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_into_iter![$(($type, $index))*];
    }
}

into_iter![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
