use crate::all_storages::AllStorages;
use crate::atomic_refcell::SharedBorrow;
use crate::entity_id::EntityId;
use crate::iter::LastId;
use crate::iter::{AbstractMut, Iter};
use crate::iter_component::IterComponent;
use crate::tracking::TrackingTimestamp;
use core::marker::PhantomData;

#[allow(missing_docs)]
pub struct IterRef<'a, T: IterComponent> {
    pub(crate) iter: Iter<T::Storage<'a>>,
    pub(crate) _all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) _borrow: T::Borrow<'a>,
}

impl<'a, T: IterComponent> Iterator for IterRef<'a, T>
where
    T::Storage<'a>: AbstractMut,
{
    type Item = <T::Storage<'a> as AbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a, T: IterComponent> LastId for IterRef<'a, T>
where
    T::Storage<'a>: AbstractMut,
{
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        self.iter.last_id()
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        self.iter.last_id_back()
    }
}

#[allow(missing_docs)]
pub struct IntoIterRef<'a, T: IterComponent> {
    pub(crate) all_storages: &'a AllStorages,
    pub(crate) all_borrow: Option<SharedBorrow<'a>>,
    pub(crate) current: TrackingTimestamp,
    pub(crate) phantom: PhantomData<T>,
}

impl<'a, T: IterComponent> IntoIterRef<'a, T> {
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
    pub fn iter(&mut self) -> IterRef<'_, T> {
        T::into_iter(self.all_storages, self.all_borrow.clone(), self.current).unwrap()
    }
}

impl<'a, 'b, T: IterComponent> IntoIterator for &'b mut IntoIterRef<'a, T>
where
    T::Storage<'b>: AbstractMut,
{
    type Item = <T::Storage<'b> as AbstractMut>::Out;
    type IntoIter = IterRef<'b, T>;

    #[inline]
    #[track_caller]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
