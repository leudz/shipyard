use crate::component::Component;
use crate::entity_id::EntityId;
use crate::not::Not;
use crate::optional::Optional;
use crate::or::OrWindow;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::{Inserted, InsertedOrModified, Modified};
use crate::{track, OneOfTwo};

/// Trait deciding what type a Shiperator will return when iterated.
///
/// This is often `&T` or `&mut T`.
pub trait ShiperatorOutput {
    /// The type returned by the Shiperator.
    type Out;
}

impl<'tmp, T: Component> ShiperatorOutput for FullRawWindow<'tmp, T> {
    type Out = &'tmp T;
}

macro_rules! impl_shiperator_output_no_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorOutput for FullRawWindowMut<'tmp, T, $track> {
                type Out = &'tmp mut T;
            }
        )+
    }
}

impl_shiperator_output_no_mut![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];

macro_rules! impl_shiperator_output_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorOutput for FullRawWindowMut<'tmp, T, $track> {
                type Out = Mut<'tmp, T>;
            }
        )+
    }
}

impl_shiperator_output_mut![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<T: ShiperatorOutput> ShiperatorOutput for Inserted<T> {
    type Out = T::Out;
}

impl<T: ShiperatorOutput> ShiperatorOutput for Modified<T> {
    type Out = T::Out;
}

impl<T: ShiperatorOutput> ShiperatorOutput for InsertedOrModified<T> {
    type Out = T::Out;
}

impl<'tmp, T: Component> ShiperatorOutput for Not<FullRawWindow<'tmp, T>> {
    type Out = ();
}

impl<'tmp, T: Component, Track> ShiperatorOutput for Not<FullRawWindowMut<'tmp, T, Track>> {
    type Out = ();
}

macro_rules! impl_shiperator_output_tracking_not {
    ($($type: ident)+) => {$(
        impl<'tmp, T: ShiperatorOutput> ShiperatorOutput for Not<$type<T>> {
            type Out = T::Out;
        }
    )+};
}

impl_shiperator_output_tracking_not![Inserted Modified InsertedOrModified];

impl<T: ShiperatorOutput, U: ShiperatorOutput> ShiperatorOutput for OrWindow<(T, U)> {
    type Out = OneOfTwo<T::Out, U::Out>;
}

impl<'tmp> ShiperatorOutput for &'tmp [EntityId] {
    type Out = EntityId;
}

impl<'tmp, T: Component> ShiperatorOutput for Optional<FullRawWindow<'tmp, T>> {
    type Out = Option<&'tmp T>;
}

impl<'tmp, T: Component, Track> ShiperatorOutput for Option<FullRawWindowMut<'tmp, T, Track>>
where
    FullRawWindowMut<'tmp, T, Track>: ShiperatorOutput,
{
    type Out = Option<<FullRawWindowMut<'tmp, T, Track> as ShiperatorOutput>::Out>;
}
