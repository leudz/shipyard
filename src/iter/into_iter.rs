use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iter::Iter;
use super::mixed::Mixed;
#[cfg(feature = "parallel")]
use super::par_iter::ParIter;
use super::tight::Tight;
use crate::storage::EntityId;
use core::ptr;

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
                let mut smallest = core::usize::MAX;
                let mut smallest_dense = ptr::null();
                let mut mask: u16 = 0;

                if let Some((len, is_exact)) = self.$index1.len() {
                    smallest = len;
                    smallest_dense = self.$index1.dense();
                    if is_exact {
                        mask = 1 << $index1;
                    }
                }

                $(
                    if let Some((len, is_exact)) = self.$index.len() {
                        if is_exact {
                            if len < smallest {
                                smallest = len;
                                smallest_dense = self.$index.dense();
                                mask = 1 << $index;
                            }
                        } else {
                            if len < smallest {
                                smallest = len;
                                smallest_dense = self.$index.dense();
                                mask = 0;
                            }
                        }
                    }
                )+

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
