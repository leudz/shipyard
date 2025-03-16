use crate::atomic_refcell::SharedBorrow;
use crate::iter::{Shiperator, ShiperatorCaptain, ShiperatorOutput, ShiperatorSailor};
use crate::iter_component::IterComponent;
use crate::sparse_set::RawEntityIdAccess;
use core::marker::PhantomData;

#[allow(missing_docs)]
pub struct IntoIterRef<'a, T: IterComponent> {
    pub(crate) shiperator: T::Shiperator<'a>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _borrow: T::Borrow<'a>,
    pub(crate) entities: RawEntityIdAccess,
    pub(crate) is_exact_sized: bool,
    pub(crate) end: usize,
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
    pub fn iter(&'b mut self) -> Shiperator<T::Shiperator<'b>>
    where
        for<'any> <T as IterComponent>::Shiperator<'any>: Clone,
    {
        let shiperator = self.shiperator.clone();

        Shiperator {
            // SAFETY: We shorten the lifetime here. To me this is okay.
            //         IntoIterRef only works with SparseSet, its shiperator doesn't contain any reference.
            //         All components are 'static so this transmute shouldn't allow a shorter lifetime to be stored.
            shiperator: unsafe {
                core::mem::transmute::<T::Shiperator<'a>, T::Shiperator<'b>>(shiperator)
            },
            entities: self.entities.clone(),
            is_exact_sized: self.is_exact_sized,
            start: 0,
            end: self.end,
        }
    }
}

impl<'a, 'b, T: IterComponent> IntoIterator for &'b mut IntoIterRef<'a, T>
where
    <T as IterComponent>::Shiperator<'b>: ShiperatorCaptain + ShiperatorSailor,
    for<'any> <T as IterComponent>::Shiperator<'any>: Clone,
{
    type Item = <T::Shiperator<'b> as ShiperatorOutput>::Out;
    type IntoIter = Shiperator<T::Shiperator<'b>>;

    #[inline]
    #[track_caller]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
