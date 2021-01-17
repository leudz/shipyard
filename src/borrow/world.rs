use super::{Borrow, FakeBorrow, IntoBorrow};
use crate::atomic_refcell::Ref;
use crate::error;
use crate::view::AllStoragesViewMut;
use crate::world::World;

pub trait IntoWorldBorrow {
    type Borrow: for<'a> WorldBorrow<'a>;
}

/// Allows a type to be borrowed by [`World::borrow`], [`World::run`] and worklaods.
pub trait WorldBorrow<'a> {
    type View;

    /// This function is where the actual borrowing happens.
    fn world_borrow(world: &'a World) -> Result<Self::View, error::GetStorage>
    where
        Self: Sized;
}

pub struct AllStoragesMutBorrower;

impl IntoWorldBorrow for AllStoragesViewMut<'_> {
    type Borrow = AllStoragesMutBorrower;
}

impl<'a> WorldBorrow<'a> for AllStoragesMutBorrower {
    type View = AllStoragesViewMut<'a>;

    #[inline]
    fn world_borrow(world: &'a World) -> Result<Self::View, error::GetStorage> {
        world
            .all_storages
            .borrow_mut()
            .map(AllStoragesViewMut)
            .map_err(error::GetStorage::AllStoragesBorrow)
    }
}

pub struct FakeBorrower<T>(T);

impl<T: 'static> IntoWorldBorrow for FakeBorrow<T> {
    type Borrow = FakeBorrower<T>;
}

impl<T: 'static> WorldBorrow<'_> for FakeBorrower<T> {
    type View = FakeBorrow<T>;

    #[inline]
    fn world_borrow(_: &World) -> Result<Self::View, error::GetStorage> {
        Ok(FakeBorrow::new())
    }
}

impl<T: IntoBorrow> IntoWorldBorrow for T {
    type Borrow = T::Borrow;
}

impl<'a, S: Borrow<'a>> WorldBorrow<'a> for S {
    type View = S::View;

    fn world_borrow(world: &'a World) -> Result<Self::View, error::GetStorage>
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
