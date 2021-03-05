use super::abstract_mut::FastAbstractMut;
use super::iter::FastIter;
use super::mixed::FastMixed;
#[cfg(feature = "rayon")]
use super::par_iter::FastParIter;
use super::tight::FastTight;
use crate::entity_id::EntityId;
use crate::iter::abstract_mut::AbstractMut;
use crate::iter::into_abstract::IntoAbstract;
use crate::sparse_set::SparseSet;
use crate::type_id::TypeId;
use core::ptr;

const ACCESS_FACTOR: usize = 3;

/// Trait used to create iterators.  
/// Yields `&mut T` for mutable components. Doesn't work with storage tracking modification.
///
/// `std::iter::IntoIterator` can't be used directly because of conflicting implementation.  
/// This trait serves as substitute.
pub trait IntoFastIter {
    #[allow(missing_docs)]
    type IntoIter: Iterator;
    #[cfg(feature = "rayon")]
    #[allow(missing_docs)]
    type IntoParIter;

    /// Returns an iterator over `SparseSet`.  
    /// Panics if one of the storage is tracking modification.  
    /// You can check if a `SparseSet` is tracking modification with [`SparseSet::is_tracking_modification`].
    ///
    /// [`iter`] can be used for storage tracking modification.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, IntoFastIter, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    /// entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    /// (&mut usizes, &u32s).fast_iter().for_each(|(x, &y)| {
    ///     *x += y as usize;
    /// });
    /// ```
    /// [`SparseSet::is_tracking_modification`]: crate::SparseSet::is_tracking_modification()
    /// [`iter`]: crate::IntoIter
    fn fast_iter(self) -> Self::IntoIter;
    /// Returns an iterator over `SparseSet`, its order is based on `D`.  
    /// Panics if one of the storage is tracking modification.  
    /// You can check if a `SparseSet` is tracking modification with [`SparseSet::is_tracking_modification`].
    ///
    /// [`iter_by`] can be used for storage tracking modification.
    ///
    /// [`SparseSet::is_tracking_modification`]: crate::SparseSet::is_tracking_modification()
    /// [`iter_by`]: crate::IntoIter
    fn fast_iter_by<D: 'static>(self) -> Self::IntoIter;
    /// Returns a parallel iterator over `SparseSet`.  
    /// Panics if one of the storage is tracking modification.  
    /// You can check if a `SparseSet` is tracking modification with [`SparseSet::is_tracking_modification`].
    ///
    /// [`par_iter`] can be used for storage tracking modification.
    ///
    /// ### Example
    /// ```
    /// use rayon::prelude::ParallelIterator;
    /// use shipyard::{EntitiesViewMut, IntoFastIter, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    /// entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    /// (&mut usizes, &u32s).fast_par_iter().for_each(|(x, &y)| {
    ///     *x += y as usize;
    /// });
    /// ```
    /// [`SparseSet::is_tracking_modification`]: crate::SparseSet::is_tracking_modification()
    /// [`par_iter`]: crate::IntoIter
    #[cfg(feature = "rayon")]
    fn fast_par_iter(self) -> Self::IntoParIter;
}

impl<T: IntoAbstract> IntoFastIter for T
where
    T::AbsView: FastAbstractMut,
    <T::AbsView as AbstractMut>::Index: Clone,
{
    type IntoIter = FastIter<T::AbsView>;
    #[cfg(feature = "rayon")]
    type IntoParIter = FastParIter<T::AbsView>;

    #[inline]
    fn fast_iter(self) -> Self::IntoIter {
        if !self.is_tracking_insertion() && !self.is_tracking_modification()
            || self.len().map(|(_, is_exact)| !is_exact).unwrap_or(true)
        {
            match self.len() {
                Some((len, true)) => FastIter::Tight(FastTight {
                    current: 0,
                    end: len,
                    storage: self.into_abstract(),
                }),
                Some((len, false)) => FastIter::Mixed(FastMixed {
                    indices: self.dense(),
                    storage: self.into_abstract(),
                    current: 0,
                    end: len,
                    mask: 0,
                    last_id: EntityId::dead(),
                }),
                None => FastIter::Tight(FastTight {
                    current: 0,
                    end: 0,
                    storage: self.into_abstract(),
                }),
            }
        } else {
            panic!("fast_iter can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
        }
    }
    #[inline]
    fn fast_iter_by<D: 'static>(self) -> Self::IntoIter {
        self.fast_iter()
    }
    #[cfg(feature = "rayon")]
    #[inline]
    fn fast_par_iter(self) -> Self::IntoParIter {
        self.fast_iter().into()
    }
}

impl<T: IntoAbstract> IntoFastIter for (T,)
where
    T::AbsView: FastAbstractMut,
    <T::AbsView as AbstractMut>::Index: From<usize> + Clone,
{
    type IntoIter = FastIter<(T::AbsView,)>;
    #[cfg(feature = "rayon")]
    type IntoParIter = FastParIter<(T::AbsView,)>;

    #[inline]
    fn fast_iter(self) -> Self::IntoIter {
        if !self.0.is_tracking_insertion() && !self.0.is_tracking_modification()
            || self.0.len().map(|(_, is_exact)| !is_exact).unwrap_or(true)
        {
            match self.0.len() {
                Some((len, true)) => FastIter::Tight(FastTight {
                    current: 0,
                    end: len,
                    storage: (self.0.into_abstract(),),
                }),
                Some((len, false)) => FastIter::Mixed(FastMixed {
                    indices: self.0.dense(),
                    storage: (self.0.into_abstract(),),
                    current: 0,
                    end: len,
                    mask: 0,
                    last_id: EntityId::dead(),
                }),
                None => FastIter::Tight(FastTight {
                    current: 0,
                    end: 0,
                    storage: (self.0.into_abstract(),),
                }),
            }
        } else {
            panic!("fast_iter can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
        }
    }
    #[inline]
    fn fast_iter_by<D: 'static>(self) -> Self::IntoIter {
        self.fast_iter()
    }
    #[cfg(feature = "rayon")]
    #[inline]
    fn fast_par_iter(self) -> Self::IntoParIter {
        self.fast_iter().into()
    }
}

macro_rules! impl_into_iter {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))+) => {
        impl<$type1: IntoAbstract, $($type: IntoAbstract),+> IntoFastIter for ($type1, $($type,)+) where $type1::AbsView: FastAbstractMut, $($type::AbsView: FastAbstractMut,)+ <$type1::AbsView as AbstractMut>::Index: From<usize> + Clone, $(<$type::AbsView as AbstractMut>::Index: From<usize> + Clone),+ {
            type IntoIter = FastIter<($type1::AbsView, $($type::AbsView,)+)>;
            #[cfg(feature = "rayon")]
            type IntoParIter = FastParIter<($type1::AbsView, $($type::AbsView,)+)>;

            #[allow(clippy::drop_copy)]
            fn fast_iter(self) -> Self::IntoIter {
                if self.$index1.is_tracking_insertion() || self.$index1.is_tracking_modification()
                    && self.$index1.len().map(|(_, is_exact)| is_exact).unwrap_or(false)
                {
                    panic!("fast_iter can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
                }

                let type_ids = [self.$index1.type_id(), $(self.$index.type_id()),+];
                let mut smallest = core::usize::MAX;
                let mut smallest_dense = ptr::null();
                let mut mask: u16 = 0;
                let mut factored_len = core::usize::MAX;

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

                $(
                    if self.$index.is_tracking_insertion() || self.$index.is_tracking_modification()
                        && self.$index.len().map(|(_, is_exact)| is_exact).unwrap_or(false)
                    {
                        panic!("fast_iter can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
                    }

                    if let Some((len, is_exact)) = self.$index.len() {
                        if is_exact {
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
                )+

                drop(factored_len);

                if smallest == core::usize::MAX {
                    FastIter::Mixed(FastMixed {
                        current: 0,
                        end: 0,
                        mask,
                        indices: smallest_dense,
                        last_id: EntityId::dead(),
                        storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                    })
                } else {
                    FastIter::Mixed(FastMixed {
                        current: 0,
                        end: smallest,
                        mask,
                        indices: smallest_dense,
                        last_id: EntityId::dead(),
                        storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                    })
                }
            }
            fn fast_iter_by<Driver: 'static>(self) -> Self::IntoIter {
                if self.$index1.is_tracking_insertion() || self.$index1.is_tracking_modification()
                    && self.$index1.len().map(|(_, is_exact)| is_exact).unwrap_or(false)
                {
                    panic!("fast_iter_by can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
                }

                let type_id = TypeId::of::<SparseSet<Driver>>();
                let mut found = false;
                let mut smallest = core::usize::MAX;
                let mut smallest_dense = ptr::null();
                let mut mask: u16 = 0;

                if self.$index1.type_id() == type_id {
                    found = true;

                    match self.$index1.len() {
                        Some((len, is_exact)) => {
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
                    if self.$index.is_tracking_insertion() || self.$index.is_tracking_modification()
                        && self.$index.len().map(|(_, is_exact)| is_exact).unwrap_or(false)
                    {
                        panic!("fast_iter_by can't be used with storage is tracking modification except if you iterate on Inserted or Modified.");
                    }

                    if !found && self.$index.type_id() == type_id {
                        found = true;

                        match self.$index.len() {
                            Some((len, is_exact)) => {
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
                    if smallest == core::usize::MAX {
                        FastIter::Mixed(FastMixed {
                            current: 0,
                            end: 0,
                            mask,
                            indices: smallest_dense,
                            last_id: EntityId::dead(),
                            storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                        })
                    } else {
                        FastIter::Mixed(FastMixed {
                            current: 0,
                            end: smallest,
                            mask,
                            indices: smallest_dense,
                            last_id: EntityId::dead(),
                            storage: (self.$index1.into_abstract(), $(self.$index.into_abstract(),)+),
                        })
                    }
                } else {
                    self.fast_iter()
                }
            }
            #[cfg(feature = "rayon")]
            #[inline]
            fn fast_par_iter(self) -> Self::IntoParIter {
                self.fast_iter().into()
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
