use crate::atomic_refcell::SharedBorrow;
use crate::iter::{RawEntityIdAccess, Shiper, Shiperator};
use crate::iter_component::IterComponent;
use core::marker::PhantomData;

#[allow(missing_docs)]
pub struct IntoIterRef<'a, T: IterComponent> {
    pub(crate) shiperator: T::Shiperator<'a>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _borrow: T::Borrow<'a>,
    /// Entities each iterator starts from, never consumed.
    pub(crate) entities: RawEntityIdAccess,
    pub(crate) is_exact_sized: bool,
    pub(crate) phantom: PhantomData<T>,
}

impl<'a, 'b, T: IterComponent> IntoIterRef<'a, T> {
    /// Borrow the storages and creates the iterator.
    ///
    /// `IntoIterator` can also be used using an exclusive reference to `IntoIterRef`. `for _ in &mut iter`
    ///
    /// ### Borrows
    ///
    /// - storage (exclusive or shared)
    ///
    /// ### Panics
    ///
    /// - Storage borrow failed.
    #[inline]
    #[track_caller]
    pub fn iter(&'b mut self) -> Shiper<T::Shiperator<'b>>
    where
        for<'any> <T as IterComponent>::Shiperator<'any>: Clone,
    {
        // We clone the shiperator we saved in its initial state, never one a previous iterator
        // moved to another segment.
        let shiperator = self.shiperator.clone();

        // SAFE: We only shorten the lifetime, 'a outlives 'b.
        // `IntoIterRef` only works with `SparseSet`, its shiperator only holds raw pointers into
        // storages we keep borrowed for 'a and into the plan our `entities` share.
        // All components are 'static so the shorter lifetime can't let anything be stored
        // that wouldn't fit the longer one.
        let shiperator =
            unsafe { core::mem::transmute::<T::Shiperator<'a>, T::Shiperator<'b>>(shiperator) };

        // Each iterator starts from its own copy of the entities: a fresh cursor and fresh
        // follow-up positions. It borrows `self` for 'b, so even sharing the plan it can't
        // outlive the borrows we hold.
        let RawEntityIdAccess {
            cursor, schedule, ..
        } = self.entities.clone();

        Shiper {
            shiperator,
            cursor,
            schedule,
            is_exact_sized: self.is_exact_sized,
        }
    }
}

impl<'a, 'b, T: IterComponent> IntoIterator for &'b mut IntoIterRef<'a, T>
where
    <T as IterComponent>::Shiperator<'b>: Shiperator,
    for<'any> <T as IterComponent>::Shiperator<'any>: Clone,
{
    type Item = <T::Shiperator<'b> as Shiperator>::Out;
    type IntoIter = Shiper<T::Shiperator<'b>>;

    #[inline]
    #[track_caller]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
