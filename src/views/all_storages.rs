use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ARef, ARefMut};
use core::ops::{Deref, DerefMut};

/// Shared view over [`AllStorages`](crate::all_storages::AllStorages).
pub struct AllStoragesView<'a>(pub(crate) ARef<'a, &'a AllStorages>);

impl Clone for AllStoragesView<'_> {
    #[inline]
    fn clone(&self) -> Self {
        AllStoragesView(self.0.clone())
    }
}

impl Deref for AllStoragesView<'_> {
    type Target = AllStorages;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<AllStorages> for AllStoragesView<'_> {
    #[inline]
    fn as_ref(&self) -> &AllStorages {
        &self.0
    }
}

/// Exclusive view over [`AllStorages`](crate::all_storages::AllStorages).
pub struct AllStoragesViewMut<'a>(pub(crate) ARefMut<'a, &'a mut AllStorages>);

impl Deref for AllStoragesViewMut<'_> {
    type Target = AllStorages;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AllStoragesViewMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_ref(&self) -> &AllStorages {
        &self.0
    }
}

impl AsMut<AllStorages> for AllStoragesViewMut<'_> {
    #[inline]
    fn as_mut(&mut self) -> &mut AllStorages {
        &mut self.0
    }
}
