mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::ShiperatorOutput;
use crate::optional::Optional;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;

/// Provides access to components when the storage drives the iteration.
pub trait ShiperatorCaptain: ShiperatorOutput {
    /// Returns the component at `index`.
    ///
    /// # Safety
    ///
    /// `index` must be less than `end`, the length given by `into_shiperator` by this Shiperator.
    unsafe fn get_captain_data(&self, index: usize) -> Self::Out;
    /// Shiperators might iterate multiple splices of `EntityId`s.\
    /// This function is called on the switch to the next slice.
    fn next_slice(&mut self);
    /// Approximation of how much time iterating this Shiperator will take.\
    /// This helps pick the fastest Shiperator when iterating multiple storages.
    ///
    /// Iterating a `Vec` of lenght 100 will return 100.
    fn sail_time(&self) -> usize;
    /// `true` when this Shiperator cannot return `None`.
    fn is_exact_sized(&self) -> bool;
    /// By default `into_shiperator` returns Shiperators that thinks they are captains.\
    /// This function is called on the ones that end up not being picked.
    fn unpick(&mut self);
}

impl<'tmp, T: Component> ShiperatorCaptain for FullRawWindow<'tmp, T> {
    #[inline]
    unsafe fn get_captain_data(&self, index: usize) -> Self::Out {
        &*self.data.add(index)
    }

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        self.dense_len
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        true
    }

    #[inline]
    fn unpick(&mut self) {}
}

macro_rules! impl_shiperator_captain_no_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorCaptain for FullRawWindowMut<'tmp, T, $track> {
                #[inline]
                unsafe fn get_captain_data(&self, index: usize) -> Self::Out {
                    &mut *self.data.add(index)
                }

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                fn unpick(&mut self) {}
            }
        )+
    }
}

impl_shiperator_captain_no_mut![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];

macro_rules! impl_shiperator_captain_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorCaptain for FullRawWindowMut<'tmp, T, $track> {
                #[inline]
                unsafe fn get_captain_data(&self, index: usize) -> Self::Out {
                    Mut {
                        flag: Some(&mut *self.modification_data.add(index)),
                        current: self.current,
                        data: &mut *self.data.add(index),
                    }
                }

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                fn unpick(&mut self) {}
            }
        )+
    }
}

impl_shiperator_captain_mut![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp> ShiperatorCaptain for &'tmp [EntityId] {
    unsafe fn get_captain_data(&self, index: usize) -> Self::Out {
        *self.get_unchecked(index)
    }

    fn next_slice(&mut self) {}

    fn sail_time(&self) -> usize {
        self.len()
    }

    fn is_exact_sized(&self) -> bool {
        false
    }

    fn unpick(&mut self) {}
}

impl<'tmp, T: Component> ShiperatorCaptain for Optional<FullRawWindow<'tmp, T>> {
    unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
        unreachable!()
    }

    fn next_slice(&mut self) {}

    fn sail_time(&self) -> usize {
        self.0.sail_time()
    }

    fn is_exact_sized(&self) -> bool {
        false
    }

    fn unpick(&mut self) {}
}

impl<'tmp, T: Component, Track> ShiperatorCaptain for Optional<FullRawWindowMut<'tmp, T, Track>>
where
    Optional<FullRawWindowMut<'tmp, T, Track>>: ShiperatorOutput,
    FullRawWindowMut<'tmp, T, Track>: ShiperatorCaptain,
{
    unsafe fn get_captain_data(&self, _index: usize) -> Self::Out {
        unreachable!()
    }

    fn next_slice(&mut self) {}

    fn sail_time(&self) -> usize {
        self.0.sail_time()
    }

    fn is_exact_sized(&self) -> bool {
        false
    }

    fn unpick(&mut self) {}
}
