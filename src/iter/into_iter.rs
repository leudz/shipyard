use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iter::Iter;
use super::mixed::Mixed;
#[cfg(feature = "parallel")]
use super::par_iter::ParIter;
use super::tight::Tight;
use crate::entity_id::EntityId;
use crate::type_id::TypeId;
use alloc::vec::Vec;
use core::ptr;

const ACCESS_FACTOR: usize = 3;

/// Trait used to create iterators.  
///
/// `std::iter::IntoIterator` can't be used directly because of conflicting implementation.  
/// This trait serves as substitute.
pub trait IntoIter {
    #[allow(missing_docs)]
    type IntoIter: Iterator;
    #[cfg(feature = "parallel")]
    #[allow(missing_docs)]
    type IntoParIter;

    /// Returns an iterator over `SparseSet`.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
    ///
    /// (&mut usizes, &u32s).iter().for_each(|(mut x, &y)| {
    ///     x.0 += y.0 as usize;
    /// });
    /// ```
    fn iter(self) -> Self::IntoIter;
    /// Returns an iterator over `SparseSet`, its order is based on `D`.
    fn iter_by<D: 'static>(self) -> Self::IntoIter;
    /// Returns a parallel iterator over `SparseSet`.
    ///
    /// ### Example
    /// ```
    /// use rayon::prelude::ParallelIterator;
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
    ///
    /// (&mut usizes, &u32s).par_iter().for_each(|(mut x, &y)| {
    ///     x.0 += y.0 as usize;
    /// });
    /// ```
    #[cfg(feature = "parallel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
    fn par_iter(self) -> Self::IntoParIter;
}

impl<T: IntoAbstract> IntoIter for T
where
    T::AbsView: AbstractMut,
    <T::AbsView as AbstractMut>::Index: From<usize> + Clone,
{
    type IntoIter = Iter<T::AbsView>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter<T::AbsView>;

    #[inline]
    fn iter(self) -> Self::IntoIter {
        let is_exact = !(self.is_not() || self.is_or() || self.is_tracking());
        match (self.len(), is_exact) {
            (Some(len), true) => Iter::Tight(Tight {
                current: 0,
                end: len,
                storage: self.into_abstract(),
            }),
            (Some(len), false) => {
                let slice = unsafe { core::slice::from_raw_parts(self.dense(), len) };

                Iter::Mixed(Mixed {
                    rev_next_storage: self.other_dense(),
                    indices: slice.iter(),
                    storage: self.into_abstract(),
                    count: 0,
                    mask: 0,
                    last_id: EntityId::dead(),
                })
            }
            (None, _) => Iter::Tight(Tight {
                current: 0,
                end: 0,
                storage: self.into_abstract(),
            }),
        }
    }
    #[inline]
    fn iter_by<D: 'static>(self) -> Self::IntoIter {
        self.iter()
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn par_iter(self) -> Self::IntoParIter {
        self.iter().into()
    }
}

impl<T: IntoAbstract> IntoIter for (T,)
where
    T::AbsView: AbstractMut,
    <T::AbsView as AbstractMut>::Index: From<usize> + Clone,
{
    type IntoIter = Iter<(T::AbsView,)>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter<(T::AbsView,)>;

    #[inline]
    fn iter(self) -> Self::IntoIter {
        let is_exact = !(self.0.is_not() || self.0.is_or() || self.0.is_tracking());
        match (self.0.len(), is_exact) {
            (Some(len), true) => Iter::Tight(Tight {
                current: 0,
                end: len,
                storage: (self.0.into_abstract(),),
            }),
            (Some(len), false) => {
                let slice = unsafe { core::slice::from_raw_parts(self.0.dense(), len) };

                Iter::Mixed(Mixed {
                    rev_next_storage: self.0.other_dense(),
                    indices: slice.iter(),
                    storage: (self.0.into_abstract(),),
                    count: 0,
                    mask: 0,
                    last_id: EntityId::dead(),
                })
            }
            (None, _) => Iter::Tight(Tight {
                current: 0,
                end: 0,
                storage: (self.0.into_abstract(),),
            }),
        }
    }
    #[inline]
    fn iter_by<D: 'static>(self) -> Self::IntoIter {
        self.iter()
    }
    #[cfg(feature = "parallel")]
    #[inline]
    fn par_iter(self) -> Self::IntoParIter {
        self.iter().into()
    }
}

macro_rules! impl_into_iter {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))+) => {
        impl<$type1: IntoAbstract, $($type: IntoAbstract),+> IntoIter for ($type1, $($type,)+)
        where
            $type1::AbsView: AbstractMut, $($type::AbsView: AbstractMut),+,
            <$type1::AbsView as AbstractMut>::Index: From<usize> + Clone, $(<$type::AbsView as AbstractMut>::Index: From<usize> + Clone),+ {

            type IntoIter = Iter<($type1::AbsView, $($type::AbsView,)+)>;
            #[cfg(feature = "parallel")]
            type IntoParIter = ParIter<($type1::AbsView, $($type::AbsView,)+)>;

            fn iter(self) -> Self::IntoIter {
                let type_ids = [self.$index1.type_id(), $(self.$index.type_id()),+];
                let mut smallest = usize::MAX;
                let mut smallest_dense = ptr::null();
                let mut mask: u16 = 0;
                let mut factored_len = usize::MAX;

                if !self.$index1.is_or() && !self.$index1.is_not() {
                    if let Some(len) = self.$index1.len() {
                        smallest = len;
                        smallest_dense = self.$index1.dense();

                        if !self.$index1.is_tracking() {
                            factored_len = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;
                            mask = 1 << $index1;
                        } else {
                            factored_len = len * type_ids.len() * ACCESS_FACTOR;
                        }
                    }
                }

                $(
                    if !self.$index.is_or() && !self.$index.is_not() {
                        if let Some(len) = self.$index.len() {
                            if !self.$index.is_tracking() {
                                let factor = len + len * (type_ids.len() - 1) * ACCESS_FACTOR;

                                if factor < factored_len {
                                    smallest = len;
                                    smallest_dense = self.$index.dense();
                                    mask = 1 << $index;
                                    factored_len = factor;
                                }
                            } else {
                                let factor = len * type_ids.len() * ACCESS_FACTOR;

                                if factor < factored_len {
                                    smallest = len;
                                    smallest_dense = self.$index.dense();
                                    mask = 0;
                                    factored_len = factor;
                                }
                            }
                        }
                    }
                )+

                let _ = factored_len;

                if smallest == usize::MAX {
                    Iter::Mixed(Mixed {
                        count: 0,
                        mask,
                        indices: [].iter(),
                        last_id: EntityId::dead(),
                        storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                        rev_next_storage: Vec::new(),
                    })
                } else {
                    let slice = unsafe { core::slice::from_raw_parts(smallest_dense, smallest) };

                    Iter::Mixed(Mixed {
                        count: 0,
                        mask,
                        indices: slice.into_iter(),
                        last_id: EntityId::dead(),
                        storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                        rev_next_storage: Vec::new(),
                    })
                }
            }
            fn iter_by<Driver: 'static>(self) -> Self::IntoIter {
                let type_id = TypeId::of::<Driver>();
                let mut found = false;
                let mut smallest = usize::MAX;
                let mut smallest_dense = ptr::null();
                let mut mask: u16 = 0;

                if self.$index1.inner_type_id() == type_id {
                    found = true;

                    match self.$index1.len() {
                        Some(len) => {
                            let is_exact = !(self.$index1.is_not()
                                || self.$index1.is_or()
                                || self.$index1.is_tracking());
                            if is_exact {
                                smallest = len;
                                smallest_dense = self.$index1.dense();
                                mask = 1 << $index1;
                            } else {
                                smallest = len;
                                smallest_dense = self.$index1.dense();
                            }
                        }
                        None => {}
                    }
                }

                $(
                    if !found && self.$index.inner_type_id() == type_id {
                        found = true;

                        match self.$index.len() {
                            Some(len) => {
                                let is_exact = !(self.$index.is_not()
                                    || self.$index.is_or()
                                    || self.$index.is_tracking());
                                if is_exact {
                                    smallest = len;
                                    smallest_dense = self.$index.dense();
                                    mask = 1 << $index;
                                } else {
                                    smallest = len;
                                    smallest_dense = self.$index.dense();
                                }
                            }
                            None => {}
                        }
                    }
                )+

                if found {
                    if smallest == usize::MAX {
                        Iter::Mixed(Mixed {
                            count: 0,
                            mask,
                            indices: [].iter(),
                            last_id: EntityId::dead(),
                            storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            rev_next_storage: Vec::new(),
                        })
                    } else {
                        let slice = unsafe { core::slice::from_raw_parts(smallest_dense, smallest) };

                        Iter::Mixed(Mixed {
                            count: 0,
                            mask,
                            indices: slice.into_iter(),
                            last_id: EntityId::dead(),
                            storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                            rev_next_storage: Vec::new(),
                        })
                    }
                } else {
                    self.iter()
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
