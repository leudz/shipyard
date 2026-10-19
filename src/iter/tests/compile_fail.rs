//! Compile-fail checks, rustdoc runs them as doc tests.

/// Asserts that implementing `IntoShiperator` requires an `unsafe impl`, even for a complete
/// implementation delegating to a view.
///
/// ```compile_fail,E0200
/// use shipyard::iter::{IntoShiperator, RawEntityIdAccess};
/// use shipyard::{Component, View};
///
/// #[derive(Component)]
/// struct Pos(f32);
///
/// struct Positions<'a, 'v>(&'a View<'v, Pos>);
///
/// impl<'a, 'v: 'a> IntoShiperator for Positions<'a, 'v> {
///     type Shiperator = <&'a View<'v, Pos> as IntoShiperator>::Shiperator;
///
///     fn into_shiperator(self) -> (Self::Shiperator, usize, RawEntityIdAccess) {
///         self.0.into_shiperator()
///     }
///
///     fn can_captain() -> bool {
///         true
///     }
///
///     fn can_sailor() -> bool {
///         true
///     }
/// }
/// ```
struct SafeIntoShiperatorImpl;

/// Asserts that implementing `IterComponent` requires an `unsafe impl`, even for a complete
/// implementation delegating to a component.
///
/// ```compile_fail,E0200
/// use shipyard::advanced::atomic_refcell::SharedBorrow;
/// use shipyard::advanced::tracking::TrackingTimestamp;
/// use shipyard::all_storages::AllStorages;
/// use shipyard::error::GetStorage;
/// use shipyard::iter::{IterComponent, RawEntityIdAccess};
/// use shipyard::Component;
///
/// #[derive(Component)]
/// struct Pos(f32);
///
/// struct Positions;
///
/// impl IterComponent for Positions {
///     type Shiperator<'a> = <&'static Pos as IterComponent>::Shiperator<'a>;
///     type Borrow<'a> = <&'static Pos as IterComponent>::Borrow<'a>;
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
///             usize,
///             RawEntityIdAccess,
///         ),
///         GetStorage,
///     > {
///         <&Pos>::into_shiperator(all_storages, all_borrow, current)
///     }
/// }
/// ```
struct SafeIterComponentImpl;

/// Asserts that a mutable item of a `World::iter` iterator can't be held while another iterator
/// of the same `IntoIterRef` is created, both could yield the same component.
///
/// ```compile_fail,E0499
/// use shipyard::{Component, World};
///
/// #[derive(Component)]
/// struct Pos(f32);
///
/// let mut world = World::new();
/// world.add_entity(Pos(1.0));
///
/// let mut iter = world.iter::<&mut Pos>();
/// let first = iter.iter().next().unwrap();
/// let second = iter.iter().next().unwrap();
/// first.0 += second.0;
/// ```
struct HeldAcrossIter;
