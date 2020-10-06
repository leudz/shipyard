use crate::unknown_storage::UnknownStorage;
use core::any::Any;

/// Type used to [`FakeBorrow`] unique storages.
///
/// [`FakeBorrow`]: struct.FakeBorrow.html
pub struct Unique<T>(pub(crate) T);

impl<T: 'static> UnknownStorage for Unique<T> {
    fn any(&self) -> &dyn Any {
        &self.0
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        &mut self.0
    }
}
