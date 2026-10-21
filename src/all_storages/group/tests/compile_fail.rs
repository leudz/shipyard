//! Compile-fail checks, rustdoc runs them as doc tests.

/// Asserts that a type outside of shipyard can't be a group member, even with a complete
/// implementation.
///
/// ```compile_fail,E0277
/// use shipyard::advanced::StorageId;
/// use shipyard::all_storages::{AllStorages, GroupMember};
///
/// struct Foreign;
///
/// impl GroupMember for Foreign {
///     const THREAD_SAFE: bool = true;
///
///     fn storage_id() -> StorageId {
///         StorageId::of::<Foreign>()
///     }
///
///     fn get_or_insert(_: &mut AllStorages) {}
/// }
/// ```
struct ForeignGroupMember;

/// Asserts that a type outside of shipyard can't be used as a group, even with a complete
/// implementation.
///
/// ```compile_fail,E0277
/// use shipyard::all_storages::{MemberInfo, TupleGroup};
///
/// struct Foreign;
///
/// impl TupleGroup for Foreign {
///     fn members() -> Vec<MemberInfo> {
///         Vec::new()
///     }
/// }
/// ```
struct ForeignTupleGroup;

/// Asserts that a single storage can't be declared as a group, a group needs at least two members.
///
/// ```compile_fail,E0277
/// use shipyard::{Component, World};
///
/// #[derive(Component)]
/// struct A;
///
/// World::builder().with_group::<(A,)>();
/// ```
struct SingleMemberGroup;

/// Asserts that a group has to be a tuple, a bare component isn't one.
///
/// ```compile_fail,E0277
/// use shipyard::{Component, World};
///
/// #[derive(Component)]
/// struct A;
///
/// World::builder().with_group::<A>();
/// ```
struct NonTupleGroup;
