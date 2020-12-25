use super::abstract_mut::AbstractMut;
use super::into_abstract::IntoAbstract;
use super::iter::Iter;
use super::mixed::Mixed;
#[cfg(feature = "parallel")]
use super::par_iter::ParIter;
use super::tight::Tight;
use crate::entity_id::EntityId;
use crate::sparse_set::SparseSet;
use crate::type_id::TypeId;
use core::ptr;

const ACCESS_FACTOR: usize = 3;

/// Trait used to create iterators. Yields [`Mut`] for mutable components.
///
/// `std::iter::IntoIterator` can't be used directly because of conflicting implementation.  
/// This trait serves as substitute.
///
/// [`Mut`]: crate::Mut
pub trait IntoIter {
    type IntoIter: Iterator;
    #[cfg(feature = "parallel")]
    type IntoParIter;

    /// Returns an iterator over `SparseSet`.
    ///
    /// Yields [`Mut`] for mutable components.  
    /// It `deref`s to the component and will flag mutation.  
    /// [`fast_iter`] can be used if you want an iterator yielding `&mut T`, it has limitations however.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    ///         for (mut x, &y) in (&mut usizes, &u32s).iter() {
    ///             *x += y as usize;
    ///         }
    ///     },
    /// );
    /// ```
    /// [`Mut`]: crate::Mut
    /// [`fast_iter`]: crate::IntoFastIter
    fn iter(self) -> Self::IntoIter;
    /// Returns an iterator over `SparseSet`, its order is based on `D`.
    ///
    /// Returns [`Mut`] when yielding mutable components.  
    /// It `deref`s to the component and will flag mutation.  
    /// [`fast_iter_by`] can be used if you want an iterator yielding `&mut T`, it has limitations however.
    ///
    /// [`Mut`]: crate::Mut
    /// [`fast_iter_by`]: crate::IntoFastIter
    fn iter_by<D: 'static>(self) -> Self::IntoIter;
    /// Returns a parallel iterator over `SparseSet`.
    ///
    /// Yields [`Mut`] for mutable components.  
    /// It `deref`s to the component and will flag mutation.  
    /// [`fast_par_iter`] can be used if you want an iterator yielding `&mut T`, it has limitations however.
    ///
    /// ### Example
    /// ```
    /// use rayon::prelude::ParallelIterator;
    /// use shipyard::{EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut,
    ///      mut usizes: ViewMut<usize>,
    ///      mut u32s: ViewMut<u32>,| {
    ///         entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    ///
    ///         (&mut usizes, &u32s).par_iter().for_each(|(mut x, &y)| {
    ///             *x += y as usize;
    ///         });
    ///     },
    /// );
    /// ```
    /// [`Mut`]: crate::Mut
    /// [`fast_par_iter`]: crate::IntoFastIter
    #[cfg(feature = "parallel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
    fn par_iter(self) -> Self::IntoParIter;
}

impl<T: IntoAbstract> IntoIter for T
where
    <T::AbsView as AbstractMut>::Index: Clone,
{
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
    <T::AbsView as AbstractMut>::Index: From<usize> + Clone,
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
        impl<$type1: IntoAbstract, $($type: IntoAbstract),+> IntoIter for ($type1, $($type,)+) where <$type1::AbsView as AbstractMut>::Index: From<usize> + Clone, $(<$type::AbsView as AbstractMut>::Index: From<usize> + Clone),+ {
            type IntoIter = Iter<($type1::AbsView, $($type::AbsView,)+)>;
            #[cfg(feature = "parallel")]
            type IntoParIter = ParIter<($type1::AbsView, $($type::AbsView,)+)>;

            #[allow(clippy::drop_copy)]
            fn iter(self) -> Self::IntoIter {
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
            fn iter_by<Driver: 'static>(self) -> Self::IntoIter {
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
