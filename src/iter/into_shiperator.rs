mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
#[cfg(feature = "parallel")]
use crate::iter::ParShiperator;
use crate::iter::mixed::{Mixed, embed_captain_entities};
use crate::iter::planner::{self, Child, NodeMetadata};
use crate::iter::{RawEntityIdAccess, Shiper, Shiperator};
use crate::optional::Optional;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::tracking::Tracking;
use crate::views::{View, ViewMut};
use core::ptr::NonNull;

/// Creates view iterators.
///
/// `std::iter::IntoIterator` can't be used directly because of conflicting implementation.\
/// This trait serves as substitute.
pub trait IntoIter: IntoShiperator {
    /// ### Example
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
    ///
    /// (&mut usizes, &u32s).iter().for_each(|(mut x, &y)| {
    ///     x.0 += y.0 as usize;
    /// });
    /// ```
    fn iter(self) -> Shiper<Self::Shiperator>;
    /// ### Example
    /// ```
    /// use rayon::prelude::ParallelIterator;
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, ViewMut, World};
    ///
    /// #[derive(Component, Clone, Copy)]
    /// struct U32(u32);
    ///
    /// #[derive(Component)]
    /// struct USIZE(usize);
    ///
    /// let world = World::new();
    ///
    /// let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>().unwrap();
    ///
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    /// entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
    ///
    /// (&mut usizes, &u32s).par_iter().for_each(|(mut x, &y)| {
    ///     x.0 += y.0 as usize;
    /// });
    /// ```
    #[cfg(feature = "parallel")]
    #[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
    fn par_iter(self) -> ParShiperator<Self::Shiperator>;
}

impl<T: IntoShiperator> IntoIter for T
where
    <T as IntoShiperator>::Shiperator: Shiperator,
{
    #[inline]
    fn iter(self) -> Shiper<Self::Shiperator> {
        let (mut shiperator, current_len, entities) = self.into_shiperator();
        debug_assert_eq!(current_len, entities.cursor.len());

        // We're the query's root, every node has been built and nothing will pick another walk.
        let entities = planner::install_winner(&mut shiperator, entities);

        // SAFE: `IntoShiperator` is an unsafe trait, its implementation guarantees `shiperator`'s
        // installed state matches the first segment of `entities` and every follow-up has a row
        // built for `shiperator`'s layout, from the same storage borrows.
        // `install_winner` either keeps that pair or installs the first grouped segment on
        // `shiperator` and returns that segment's entities, with the plan `grouped_entities`
        // built from `shiperator`'s own description and the metadata of these same borrows.
        unsafe { Shiper::new(shiperator, entities) }
    }

    #[cfg(feature = "parallel")]
    #[inline]
    fn par_iter(self) -> ParShiperator<Self::Shiperator> {
        ParShiperator(self.iter())
    }
}

/// Turns a view into a Shiperator.
///
/// Calling [`iter`](IntoIter::iter), [`par_iter`](IntoIter::par_iter) or
/// [`into_shiperator`](IntoShiperator::into_shiperator) is always safe.\
/// Implementing this trait is only needed for custom views, `#[derive(IntoIter)]` generates it.
/// The simplest implementation delegates to an existing one, a view or a tuple of views,
/// and returns its results unchanged:
///
/// ```
/// use shipyard::iter::{IntoShiperator, RawEntityIdAccess};
/// use shipyard::{Component, IntoIter, View, World};
///
/// #[derive(Component)]
/// struct Pos(f32);
///
/// struct Positions<'a, 'v>(&'a View<'v, Pos>);
///
/// // SAFE: We return the view's own Shiperator and entities, unchanged.
/// // `Positions` holds the view's borrow for 'a, as long as the Shiperator.
/// unsafe impl<'a, 'v: 'a> IntoShiperator for Positions<'a, 'v> {
///     type Shiperator = <&'a View<'v, Pos> as IntoShiperator>::Shiperator;
///
///     fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
///         self.0.into_shiperator()
///     }
///
///     fn can_captain() -> bool {
///         <&'a View<'v, Pos> as IntoShiperator>::can_captain()
///     }
///
///     fn can_sailor() -> bool {
///         <&'a View<'v, Pos> as IntoShiperator>::can_sailor()
///     }
/// }
///
/// let mut world = World::new();
/// world.add_entity(Pos(1.0));
/// world.add_entity(Pos(2.0));
///
/// let positions = world.borrow::<View<Pos>>().unwrap();
/// let sum: f32 = Positions(&positions).iter().map(|pos| pos.0).sum();
/// assert_eq!(sum, 3.0);
/// ```
///
/// ### Safety
///
/// Iterators trust what [`into_shiperator`](IntoShiperator::into_shiperator) returns without
/// checking it. Implementations must uphold all of the following, for the returned
/// Shiperator, its clones and every segment state they install:
///
/// - The Shiperator, the length and the [`RawEntityIdAccess`] describe the same query, built
///   from the same storage borrows. The length is the number of candidates of the access's
///   first segment. The access, follow-ups and grouping metadata included, must come from the
///   `into_shiperator` of the views the Shiperator is made of. An access returned by another
///   call can't be paired with an unrelated Shiperator, even one of the same type.
/// - [`captain_indices_of`](Shiperator::captain_indices_of) resolves a candidate's local index
///   to the components of that candidate's entity.
///   [`sailor_indices_of`](Shiperator::sailor_indices_of) and
///   [`storage_index`](Shiperator::storage_index) return absolute indices. Whether an entity is
///   a match doesn't depend on the segment currently traversed.
/// - The storages and the entities the access points to stay allocated and unmodified as long
///   as the Shiperator, its clones, the iterators built from them and the items they yield are
///   alive. Holding the borrows they come from for the Shiperator's lifetime does it.
/// - Two candidates the traversal accepts, across follow-up segments and split halves, never
///   yield overlapping mutable outputs. A clone resolves candidates exactly like its original
///   and each candidate is handed to a single one of them, cloning doesn't allow a mutable
///   component to be yielded twice.
/// - [`node_count`](Shiperator::node_count), [`describe`](Shiperator::describe) and
///   [`enter_segment`](Shiperator::enter_segment) agree on the Shiperator's layout and states,
///   [`is_exact_sized`](Shiperator::is_exact_sized) only returns `true` when no candidate of the
///   current segment can be rejected. Entering a segment only installs state: it creates no
///   reference to a component and calls no consumer callback.
pub unsafe trait IntoShiperator {
    #[allow(missing_docs)]
    type Shiperator;

    /// Returns the Shiperator, the number of candidates in its first segment
    /// and the entities driving the iteration.
    ///
    /// The Shiperator's state matches the first segment, follow-up segments install their own.\
    /// The returned length doesn't count follow-up segments,
    /// the total number of candidates can be larger.
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess);
    /// Returns `true` if the Shiperator can be a captain.
    fn can_captain() -> bool;
    /// Returns `true` if the Shiperator can be a sailor.
    fn can_sailor() -> bool;
}

// SAFE: The window and its entities are both read from the view's storage, which stays borrowed
// for 'tmp. A window resolves `segment_start + local_index`, the index of the candidate's own
// component, and its metadata describes this same storage. It only yields shared references.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for &'tmp View<'v, T, Track>
{
    type Shiperator = FullRawWindow<'tmp, T, Track>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let window = FullRawWindow::from_view(self);
        let len = window.len();
        let iter = window
            .entity_iter()
            .with_metadata(NodeMetadata::window(self.sparse_set));

        (window, len, iter)
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

// SAFE: Same as `&View`, the storage stays borrowed for 'tmp and only shared references are yielded.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for &'tmp ViewMut<'v, T, Track>
{
    type Shiperator = FullRawWindow<'tmp, T, Track>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let window = FullRawWindow::from_view_mut(self);
        let len = window.len();
        let iter = window
            .entity_iter()
            .with_metadata(NodeMetadata::window(self.sparse_set));

        (window, len, iter)
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

// SAFE: Same as `&View`, with the storage exclusively borrowed for 'tmp. Each component belongs
// to a single entity and segments never walk an entity twice without a coverage record rejecting
// it, so each mutable component is yielded at most once.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for &'tmp mut ViewMut<'v, T, Track>
{
    type Shiperator = FullRawWindowMut<'tmp, T, Track>;

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let metadata = NodeMetadata::window(self.sparse_set);
        let window = FullRawWindowMut::new(self);
        let len = window.len();
        let iter = window.entity_iter().with_metadata(metadata);

        (window, len, iter)
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        true
    }
}

// SAFE: The slice is its own single segment, borrowed for 'tmp. It yields entity ids by value.
unsafe impl<'tmp> IntoShiperator for &'tmp [EntityId] {
    type Shiperator = &'tmp [EntityId];

    #[inline]
    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let len = self.len();
        // SAFE: The slice is borrowed for `'tmp` and the access is only used by iterators
        // over this Shiperator, which hold that borrow.
        let iter = unsafe { RawEntityIdAccess::new(NonNull::from(self).cast(), len) };

        (self, len, iter)
    }

    #[inline]
    fn can_captain() -> bool {
        true
    }

    #[inline]
    fn can_sailor() -> bool {
        false
    }
}

// SAFE: We wrap the view's own results. `Optional` can't captain so it's only ever looked up,
// its entities are marked filtered and never drive a grouped walk.
unsafe impl<'tmp, 'v: 'tmp, T: Component> IntoShiperator for Optional<&'tmp View<'v, T>> {
    type Shiperator = Optional<FullRawWindow<'tmp, T, T::Tracking>>;

    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator, len, entities) = self.0.into_shiperator();

        (Optional(shiperator), len, entities.filtered())
    }

    fn can_captain() -> bool {
        false
    }

    fn can_sailor() -> bool {
        true
    }
}

// SAFE: See `Optional<&View>`.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Optional<&'tmp ViewMut<'v, T, Track>>
{
    type Shiperator = Optional<FullRawWindow<'tmp, T, Track>>;

    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator, len, entities) = self.0.into_shiperator();

        (Optional(shiperator), len, entities.filtered())
    }

    fn can_captain() -> bool {
        false
    }

    fn can_sailor() -> bool {
        true
    }
}

// SAFE: See `Optional<&View>`, the inner `&mut ViewMut` upholds the uniqueness of mutable outputs.
unsafe impl<'tmp, 'v: 'tmp, T: Component, Track: Tracking> IntoShiperator
    for Optional<&'tmp mut ViewMut<'v, T, Track>>
{
    type Shiperator = Optional<FullRawWindowMut<'tmp, T, Track>>;

    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        let (shiperator, len, entities) = self.0.into_shiperator();

        (Optional(shiperator), len, entities.filtered())
    }

    fn can_captain() -> bool {
        false
    }

    fn can_sailor() -> bool {
        true
    }
}

// SAFE: We return `T`'s results unchanged, its implementation upholds the contract.
unsafe impl<T: IntoShiperator> IntoShiperator for (T,) {
    type Shiperator = T::Shiperator;

    fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
        self.0.into_shiperator()
    }

    fn can_captain() -> bool {
        T::can_captain()
    }

    fn can_sailor() -> bool {
        T::can_sailor()
    }
}

// It's not currently possible to have a final non repeating '+'
// https://github.com/rust-lang/rust/issues/18700
macro_rules! strip_plus {
    (+ $($rest: tt)*) => {
        $($rest)*
    }
}

// This is used in mixed.rs
pub(crate) use strip_plus;

macro_rules! impl_into_shiperator_tuple {
    ($(($type: ident, $index: tt))+) => {
        // SAFE: Each child's results come from its own implementation, which upholds the contract.
        // The children are borrowed separately so their mutable outputs can't overlap.
        // `Mixed` keeps the captain's entities and follow-ups, embedded under rows built from its
        // own description, and looks the other children up by entity.
        // `planner::select` only reads the children's metadata, nothing is installed here.
        unsafe impl<$($type: IntoShiperator),+> IntoShiperator for ($($type,)+) where $(<$type as IntoShiperator>::Shiperator: Shiperator),+ {
            type Shiperator = Mixed<($($type::Shiperator,)+)>;

            #[inline]
            #[track_caller]
            fn into_shiperator(
                self,
            ) -> (Self::Shiperator, usize, RawEntityIdAccess) {
                let mut shiperators = ($(self.$index.into_shiperator(),)+);

                let can_captains = ($(
                    $type::can_captain(),
                )+);

                let can_any_captain = $(
                    can_captains.$index
                )||+;

                if !can_any_captain {
                    panic!("Unable to build a Shiperator: None of the views can be a Captain.")
                }

                let can_sailors = ($(
                    $type::can_sailor(),
                )+);

                $(
                    if !can_captains.$index && !can_sailors.$index {
                        panic!("Unable to build a Shiperator: View at index {} can neither be a Captain nor a Sailor.", $index)
                    }
                )+

                let unable_sailor = strip_plus!($(
                    + (!can_sailors.$index as usize)
                )+);

                if unable_sailor > 1 {
                    panic!("Unable to build a Shiperator: Multiple views are unable to be Sailors.")
                }

                let mut children = [$(
                    Child::new(
                        can_captains.$index,
                        can_sailors.$index,
                        shiperators.$index.0.sail_time(),
                        &mut shiperators.$index.2,
                    ),
                )+];
                let selection = planner::select(&mut children);

                // Preorder position of each child's states in the Mixed's rows.
                let mut next_slot = 1;
                let slots = ($({
                    let slot = next_slot;
                    next_slot += <$type::Shiperator as Shiperator>::node_count();
                    slot
                },)+);
                let _ = next_slot;

                // We always install the ordinary walk, our parent may drive through it.
                // A cheaper grouped walk, through our groups or our children's, is only installed
                // by the query's root, `IntoIter::iter`.
                let captain = selection.ordinary.captain;
                let mut captain_mask = 0;
                let mut captain_slot = 0;
                let mut len = 0;
                let mut entity_iter = RawEntityIdAccess::empty();

                $(
                    if captain == $index {
                        captain_mask = 1 << $index;
                        captain_slot = slots.$index;
                        len = shiperators.$index.1;
                        entity_iter = shiperators.$index.2;
                    }
                )+

                let mixed = Mixed::new(($(shiperators.$index.0,)+), captain_mask);
                let entity_iter = embed_captain_entities(&mixed, captain_slot, entity_iter)
                    .with_metadata(selection.into_metadata());

                (mixed, len, entity_iter)
            }

            #[inline]
            fn can_captain() -> bool {
                $(
                    $type::can_captain()
                )||+
            }

            #[inline]
            fn can_sailor() -> bool {
                $(
                    $type::can_sailor()
                )&&+
            }
        }
    };
}

macro_rules! into_shiperator_tuple {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_into_shiperator_tuple![$(($type, $index))*];
        into_shiperator_tuple![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_into_shiperator_tuple![$(($type, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
into_shiperator_tuple![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
#[cfg(feature = "extended_tuple")]
into_shiperator_tuple![
    (A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)
    (K, 10) (L, 11) (M, 12) (N, 13) (O, 14) (P, 15) (Q, 16) (R, 17) (S, 18) (T, 19)
    (U, 20) (V, 21) (W, 22) (X, 23) (Y, 24) (Z, 25) (AA, 26) (BB, 27) (CC, 28) (DD, 29)
    (EE, 30) (FF, 31)
];
