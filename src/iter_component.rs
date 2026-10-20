mod iter_ref;

pub use iter_ref::IntoIterRef;

use crate::all_storages::AllStorages;
use crate::atomic_refcell::{ExclusiveBorrow, SharedBorrow};
use crate::borrow::Borrow;
#[cfg(feature = "thread_local")]
use crate::borrow::{NonSend, NonSendSync, NonSync};
use crate::component::Component;
use crate::iter::is_exact_traversal;
use crate::iter::mixed::embed_captain_entities;
use crate::iter::planner::{self, Child, ConstructionMetadata};
use crate::iter::{Mixed, RawEntityIdAccess, Shiperator};
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::Tracking;
use crate::tracking::TrackingTimestamp;
use crate::views::{View, ViewMut};
use crate::{error, track};
use core::any::type_name;

/// Trait used as bound for [`World::iter`] and [`AllStorages::iter`].
///
/// Calling these methods is always safe.\
/// Implementing this trait is only needed for custom component sets. The simplest
/// implementation delegates to an existing one and returns its results unchanged:
///
/// ```
/// use shipyard::advanced::atomic_refcell::SharedBorrow;
/// use shipyard::advanced::tracking::TrackingTimestamp;
/// use shipyard::all_storages::AllStorages;
/// use shipyard::error::GetStorage;
/// use shipyard::iter::{ConstructionMetadata, IterComponent, RawEntityIdAccess};
/// use shipyard::{Component, World};
///
/// #[derive(Component)]
/// struct Pos(f32);
///
/// struct Positions;
///
/// // SAFE: We return `&mut Pos`'s results unchanged, guards and metadata included.
/// unsafe impl IterComponent for Positions {
///     type Shiperator<'a> = <&'static mut Pos as IterComponent>::Shiperator<'a>;
///     type Borrow<'a> = <&'static mut Pos as IterComponent>::Borrow<'a>;
///
///     fn into_shiperator<'a>(
///         all_storages: &'a AllStorages,
///         all_borrow: Option<SharedBorrow<'a>>,
///         current: TrackingTimestamp,
///     ) -> Result<
///         (
///             Self::Shiperator<'a>,
///             Option<SharedBorrow<'a>>,
///             Self::Borrow<'a>,
///             RawEntityIdAccess,
///             ConstructionMetadata,
///         ),
///         GetStorage,
///     > {
///         <&mut Pos>::into_shiperator(all_storages, all_borrow, current)
///     }
/// }
///
/// let mut world = World::new();
/// world.add_entity(Pos(1.0));
///
/// for pos in &mut world.iter::<Positions>() {
///     pos.0 += 1.0;
/// }
///
/// let sum: f32 = world.iter::<&Pos>().iter().map(|pos| pos.0).sum();
/// assert_eq!(sum, 2.0);
/// ```
///
/// ### Safety
///
/// Iterators trust what [`into_shiperator`](IterComponent::into_shiperator) returns without
/// checking it. Implementations must uphold all of the following:
///
/// - The returned Shiperator, [`RawEntityIdAccess`] and
///   [`ConstructionMetadata`](crate::iter::ConstructionMetadata) uphold
///   [`IntoShiperator`](crate::iter::IntoShiperator)'s contract, with the returned guards in place
///   of a Rust borrow: they come from the same storage borrows and agree with each other.
///   The metadata is only read while [`World::iter`] builds the query, [`IntoIterRef`] doesn't
///   keep it. The returned guards keep the group metadata it points to borrowed.
/// - The returned `Borrow<'a>` and `all_borrow` guards keep every storage the Shiperator reads
///   borrowed, exclusively when it yields mutable outputs. [`IntoIterRef`] holds them as long as
///   it and the items of its iterators are alive, nothing the Shiperator or the access points to
///   may outlive them.
/// - A conflict with another member of the same query, two exclusive borrows of one storage for
///   example, is detected and reported as an error. Acquiring the storage borrows, as the
///   built-in implementations do, detects it.
/// - For every `'b` shorter than `'a`, a `Shiperator<'a>` is also a valid `Shiperator<'b>` with
///   the same representation. The two types only differ by this lifetime and shortening it can't
///   let anything be stored that has to live for `'a`, as if `Shiperator` was covariant.
/// - Clones of the Shiperator resolve candidates exactly like the original.
///   [`IntoIterRef::iter`] builds each iterator from a fresh clone, cloning doesn't allow
///   a mutable component to be yielded twice by the same iterator.
///
/// [`World::get`]: crate::World::get
/// [`World::iter`]: crate::World::iter
pub unsafe trait IterComponent {
    #[allow(missing_docs)]
    type Shiperator<'a>;
    #[allow(missing_docs)]
    type Borrow<'a>;

    #[allow(missing_docs)]
    #[allow(clippy::type_complexity)]
    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    >;
}

pub(crate) fn into_iter<'a, T: IterComponent>(
    all_storages: &'a AllStorages,
    all_borrow: Option<SharedBorrow<'a>>,
    current: TrackingTimestamp,
) -> Result<IntoIterRef<'a, T>, error::GetStorage>
where
    <T as IterComponent>::Shiperator<'a>: Shiperator,
{
    let (mut shiperator, all_borrow, borrow, entities, metadata) =
        T::into_shiperator(all_storages, all_borrow, current)?;

    // We're the query's root, every node has been built and nothing will pick another walk.
    // The metadata ends here, every iterator starts from these entities.
    // SAFE: All three come from the `into_shiperator` call above.
    let entities = unsafe { planner::install_winner(&mut shiperator, entities, metadata) };
    let is_exact_sized = is_exact_traversal(&shiperator, &entities);

    Ok(IntoIterRef {
        // Holds the first segment's state, each iterator starts from a clone of it.
        // Our borrows keep the storages from changing layout, so the plan built from them
        // stays accurate, and `entities` keeps that plan alive.
        shiperator,
        _all_borrow: all_borrow,
        _borrow: borrow,
        entities,
        is_exact_sized,
        phantom: core::marker::PhantomData,
    })
}

// SAFE: The window and its entities are read from the view we borrowed, which we return as
// guards. A view's borrow fails when the storage is already exclusively borrowed, by another
// member of the query included. Windows only hold raw pointers into the storage, shortening
// their lifetime is like shortening a reference's. See `IntoShiperator for &View`.
unsafe impl<T: Component + Send + Sync> IterComponent for &'_ T {
    type Shiperator<'a> = FullRawWindow<'a, T, T::Tracking>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = View::<'a, T>::borrow(all_storages, all_borrow, None, current)?;
        let metadata = ConstructionMetadata::window(&view);
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Sync> IterComponent for NonSend<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T, T::Tracking>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let metadata = ConstructionMetadata::window(&view);
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Send> IterComponent for NonSync<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T, T::Tracking>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let metadata = ConstructionMetadata::window(&view);
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component> IterComponent for NonSendSync<&'_ T> {
    type Shiperator<'a> = FullRawWindow<'a, T, T::Tracking>;
    type Borrow<'a> = SharedBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<View<'a, T>>::borrow(all_storages, all_borrow, None, current)?;
        let metadata = ConstructionMetadata::window(&view);
        let (window, all_borrow, borrow) = FullRawWindow::from_owned_view(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&T`. The view is borrowed exclusively, so is the storage the mutable outputs
// come from. See `IntoShiperator for &mut ViewMut` for their uniqueness.
unsafe impl<T: Component + Send + Sync> IterComponent for &'_ mut T {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    #[track_caller]
    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = ViewMut::<'a, T>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Sync> IterComponent for NonSend<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Send> IterComponent for NonSync<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component> IterComponent for NonSendSync<&'_ mut T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, T::Tracking>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T>>::borrow(all_storages, all_borrow, None, current)?;

        if view.sparse_set.is_tracking_modification && !T::Tracking::track_modification() {
            panic!(
                "`{0}` tracks modification but trying to iterate `&mut {0}`. Use `Mut<{0}>` instead.",
                type_name::<T>()
            );
        }

        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`.
unsafe impl<T: Component + Send + Sync> IterComponent for Mut<'_, T> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view =
            ViewMut::<'a, T, track::Modification>::borrow(all_storages, all_borrow, None, current)?;
        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Sync> IterComponent for NonSend<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSend::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component + Send> IterComponent for NonSync<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

// SAFE: See `&mut T`, the wrapper's borrow also checks the thread we're on.
#[cfg(feature = "thread_local")]
unsafe impl<T: Component> IterComponent for NonSendSync<Mut<'_, T>> {
    type Shiperator<'a> = FullRawWindowMut<'a, T, track::Modification>;
    type Borrow<'a> = ExclusiveBorrow<'a>;

    fn into_shiperator<'a>(
        all_storages: &'a AllStorages,
        all_borrow: Option<SharedBorrow<'a>>,
        current: TrackingTimestamp,
    ) -> Result<
        (
            Self::Shiperator<'a>,
            Option<SharedBorrow<'a>>,
            Self::Borrow<'a>,
            RawEntityIdAccess,
            ConstructionMetadata,
        ),
        error::GetStorage,
    > {
        let view = NonSendSync::<ViewMut<'a, T, track::Modification>>::borrow(
            all_storages,
            all_borrow,
            None,
            current,
        )?;
        let (window, all_borrow, borrow, metadata) = FullRawWindowMut::new_owned(view.0);

        let entities = window.entity_iter();

        Ok((window, all_borrow, borrow, entities, metadata))
    }
}

macro_rules! impl_iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $metadata: ident, $entity_iter: ident, $index: tt))+) => {
        // SAFE: Each child's results come from its own implementation, which upholds the contract.
        // We return all their guards and our own `all_borrow`, the clone each child got shares it.
        // Children detect conflicts between themselves, see `&mut T`. `Mixed` holds the children's
        // Shiperators as is, shortening its lifetime shortens each of theirs.
        // The rest is the same as `IntoShiperator`'s tuples.
        unsafe impl<$($type: IterComponent),+> IterComponent for ($($type,)+) where $(for<'a> $type::Shiperator<'a>: Shiperator,)+ {
            type Shiperator<'a> = Mixed<($($type::Shiperator<'a>,)+)>;
            type Borrow<'a> = ($($type::Borrow<'a>,)+);

            fn into_shiperator<'a>(
                all_storages: &'a AllStorages,
                all_borrow: Option<SharedBorrow<'a>>,
                current: TrackingTimestamp,
            ) -> Result<
                (
                    Self::Shiperator<'a>,
                    Option<SharedBorrow<'a>>,
                    Self::Borrow<'a>,
                    RawEntityIdAccess,
                    ConstructionMetadata,
                ),
                error::GetStorage,
            > {
                $(
                    let ($raw_window, _, $borrow, $entity_iter, $metadata) = $type::into_shiperator(all_storages, all_borrow.clone(), current)?;
                )+

                // Owned iteration only builds windows and tuples of them,
                // they can all be captains and sailors.
                // SAFE: Each child's metadata comes from its own `into_shiperator` call, along with
                // the guards we keep in `$borrow` and return. `IterComponent`'s contract keeps the
                // storages that call borrowed, with their group metadata unmodified and readable,
                // as long as these guards live.
                let mut children = [$(
                    unsafe {
                        Child::new(true, true, $raw_window.sail_time(), &$entity_iter, $metadata)
                    },
                )+];
                let selection = planner::select(&mut children);

                // We always install the ordinary walk, our parent may drive through it.
                // A cheaper grouped walk, through our groups or our children's, is only installed
                // by the query's root, `into_iter`.
                let captain = selection.ordinary.captain;
                let mut captain_mask = 0;
                let mut captain_slot = 0;
                let mut entity_iter = RawEntityIdAccess::empty();
                // Preorder position of the current child's states in the Mixed's rows.
                let mut slot = 1;

                $(
                    if captain == $index {
                        captain_mask = 1 << $index;
                        captain_slot = slot;
                        entity_iter = $entity_iter;
                    }

                    slot += <$type::Shiperator<'a> as Shiperator>::node_count();
                )+

                let _ = slot;

                let mixed = Mixed::new(($($raw_window,)+), captain_mask);
                let entity_iter = embed_captain_entities(&mixed, captain_slot, entity_iter);

                Ok((mixed, all_borrow, ($($borrow,)+), entity_iter, selection.into_metadata()))
            }
        }
    }
}

macro_rules! iter_component {
    ($(($type: ident, $raw_window: ident, $borrow: ident, $metadata: ident, $entity_iter: ident, $index: tt))+; ($type1: ident, $raw_window1: ident, $borrow1: ident, $metadata1: ident, $entity_iter1: ident, $index1: tt) $(($queue_type: ident, $queue_raw_window: ident, $queue_borrow: ident, $queue_metadata: ident, $queue_entity_iter: ident, $queue_index: tt))*) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $metadata, $entity_iter, $index))*];
        iter_component![$(($type, $raw_window, $borrow, $metadata, $entity_iter, $index))* ($type1, $raw_window1, $borrow1, $metadata1, $entity_iter1, $index1); $(($queue_type, $queue_raw_window, $queue_borrow, $queue_metadata, $queue_entity_iter, $queue_index))*];
    };
    ($(($type: ident, $raw_window: ident, $borrow: ident, $metadata: ident, $entity_iter: ident, $index: tt))+;) => {
        impl_iter_component![$(($type, $raw_window, $borrow, $metadata, $entity_iter, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
iter_component![(A, raw_window0, borrow0, metadata0, entity_iter0, 0); (B, raw_window1, borrow1, metadata1, entity_iter1, 1) (C, raw_window2, borrow2, metadata2, entity_iter2, 2) (D, raw_window3, borrow3, metadata3, entity_iter3, 3) (E, raw_window4, borrow4, metadata4, entity_iter4, 4) (F, raw_window5, borrow5, metadata5, entity_iter5, 5) (G, raw_window6, borrow6, metadata6, entity_iter6, 6) (H, raw_window7, borrow7, metadata7, entity_iter7, 7) (I, raw_window8, borrow8, metadata8, entity_iter8, 8) (J, raw_window9, borrow9, metadata9, entity_iter9, 9)];
#[cfg(feature = "extended_tuple")]
iter_component![
    (A, raw_window0, borrow0, metadata0, entity_iter0, 0); (B, raw_window1, borrow1, metadata1, entity_iter1, 1) (C, raw_window2, borrow2, metadata2, entity_iter2, 2) (D, raw_window3, borrow3, metadata3, entity_iter3, 3) (E, raw_window4, borrow4, metadata4, entity_iter4, 4) (F, raw_window5, borrow5, metadata5, entity_iter5, 5) (G, raw_window6, borrow6, metadata6, entity_iter6, 6) (H, raw_window7, borrow7, metadata7, entity_iter7, 7) (I, raw_window8, borrow8, metadata8, entity_iter8, 8) (J, raw_window9, borrow9, metadata9, entity_iter9, 9)
    (K, raw_window10, borrow10, metadata10, entity_iter10, 10) (L, raw_window11, borrow11, metadata11, entity_iter11, 11) (M, raw_window12, borrow12, metadata12, entity_iter12, 12) (N, raw_window13, borrow13, metadata13, entity_iter13, 13) (O, raw_window14, borrow14, metadata14, entity_iter14, 14) (P, raw_window15, borrow15, metadata15, entity_iter15, 15) (Q, raw_window16, borrow16, metadata16, entity_iter16, 16) (R, raw_window17, borrow17, metadata17, entity_iter17, 17) (S, raw_window18, borrow18, metadata18, entity_iter18, 18) (T, raw_window19, borrow19, metadata19, entity_iter19, 19)
    (U, raw_window20, borrow20, metadata20, entity_iter20, 20) (V, raw_window21, borrow21, metadata21, entity_iter21, 21) (W, raw_window22, borrow22, metadata22, entity_iter22, 22) (X, raw_window23, borrow23, metadata23, entity_iter23, 23) (Y, raw_window24, borrow24, metadata24, entity_iter24, 24) (Z, raw_window25, borrow25, metadata25, entity_iter25, 25) (AA, raw_window26, borrow26, metadata26, entity_iter26, 26) (BB, raw_window27, borrow27, metadata27, entity_iter27, 27) (CC, raw_window28, borrow28, metadata28, entity_iter28, 28) (DD, raw_window29, borrow29, metadata29, entity_iter29, 29)
    (EE, raw_window30, borrow30, metadata30, entity_iter30, 30) (FF, raw_window31, borrow31, metadata31, entity_iter31, 31)
];
