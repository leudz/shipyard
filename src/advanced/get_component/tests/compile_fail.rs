//! Compile-fail checks, rustdoc runs them as doc tests.

/// Asserts that a shared component reference cannot outlive its guard.
///
/// ```compile_fail,E0597
/// use shipyard::{Component, World};
///
/// #[derive(Component)]
/// struct Position(i32);
///
/// let mut world = World::new();
/// let entity = world.add_entity(Position(1));
/// let position = {
///     let guard = world.get::<&Position>(entity).unwrap();
///     &*guard
/// };
/// assert_eq!(position.0, 1);
/// ```
struct SharedGuard;

/// Asserts that a shared projection cannot store its input reference outside the callback.
///
/// ```compile_fail,E0521
/// use shipyard::{Component, World, advanced::get_component::Ref};
///
/// #[derive(Component)]
/// struct Position(i32);
///
/// let mut world = World::new();
/// let entity = world.add_entity(Position(1));
/// let mut saved = None;
/// let guard = world.get::<&Position>(entity).unwrap();
/// let field = Ref::map(guard, |position| {
///     saved = Some(position);
///     &position.0
/// });
/// drop(field);
/// assert_eq!(saved.unwrap().0, 1);
/// ```
struct SharedProjection;

/// Asserts that a mutable component reference cannot outlive the projected guard.
///
/// ```compile_fail,E0597
/// use shipyard::{Component, World, advanced::get_component::RefMut};
///
/// #[derive(Component)]
/// struct Position(i32);
///
/// let mut world = World::new();
/// let entity = world.add_entity(Position(1));
/// let field = {
///     let guard = world.get::<&mut Position>(entity).unwrap();
///     let mut projected = RefMut::map(guard, |position| &mut position.0);
///     projected.as_mut()
/// };
/// *field = 2;
/// ```
struct MutableGuard;

/// Asserts that a mutable projection cannot store part of its input outside the callback.
///
/// ```compile_fail,E0521
/// use shipyard::{Component, World, advanced::get_component::RefMut};
///
/// #[derive(Component)]
/// struct Position([i32; 2]);
///
/// let mut world = World::new();
/// let entity = world.add_entity(Position([1, 2]));
/// let mut saved = None;
/// let guard = world.get::<&mut Position>(entity).unwrap();
/// let rest = RefMut::map(guard, |position| {
///     let (first, rest) = position.0.split_at_mut(1);
///     saved = Some(first);
///     rest
/// });
/// drop(rest);
/// saved.unwrap()[0] = 3;
/// ```
struct MutableProjection;
