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
/// use shipyard::advanced::StorageId;
/// use shipyard::all_storages::{AllStorages, TupleGroup};
///
/// struct Foreign;
///
/// impl TupleGroup for Foreign {
///     fn storage_ids() -> Vec<StorageId> {
///         Vec::new()
///     }
///
///     fn get_or_insert_storages(_: &mut AllStorages) {}
/// }
/// ```
struct ForeignTupleGroup;
