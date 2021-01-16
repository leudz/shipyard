use super::{Borrow, FakeBorrow};
use crate::atomic_refcell::Ref;
use crate::error;
use crate::view::AllStoragesViewMut;
use crate::world::World;

/// Allows a type to be borrowed by [`World::borrow`], [`World::run`] and worklaods.
pub trait WorldBorrow<'a> {
    /// This function is where the actual borrowing happens.
    fn borrow(world: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized;
}

impl<'a> WorldBorrow<'a> for AllStoragesViewMut<'a> {
    #[inline]
    fn borrow(world: &'a World) -> Result<Self, error::GetStorage> {
        world
            .all_storages
            .borrow_mut()
            .map(AllStoragesViewMut)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

impl<T: 'static> WorldBorrow<'_> for FakeBorrow<T> {
    #[inline]
    fn borrow(_: &World) -> Result<Self, error::GetStorage> {
        Ok(FakeBorrow::new())
    }
}

impl<'a, S> WorldBorrow<'a> for S
where
    S: Borrow<'a>,
{
    fn borrow(world: &'a World) -> Result<Self, error::GetStorage>
    where
        Self: Sized,
    {
        let (all_storages, all_borrow) = unsafe {
            Ref::destructure(
                world
                    .all_storages
                    .borrow()
                    .map_err(error::GetStorage::AllStoragesBorrow)?,
            )
        };

        S::borrow(all_storages, Some(all_borrow))
    }
}
