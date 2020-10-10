use crate::unknown_storage::UnknownStorage;

/// Type used to [`FakeBorrow`] unique storages.
///
/// [`FakeBorrow`]: struct.FakeBorrow.html
pub struct Unique<T>(pub(crate) T);

impl<T: 'static> UnknownStorage for Unique<T> {}
