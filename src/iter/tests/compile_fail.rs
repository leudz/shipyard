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
