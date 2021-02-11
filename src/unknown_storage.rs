use crate::entity_id::EntityId;
use crate::memory_usage::StorageMemoryUsage;
use alloc::borrow::Cow;
use core::any::Any;

pub trait SizedAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> SizedAny for T {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Defines common storage operations.
pub trait UnknownStorage: SizedAny {
    /// Casts to `&dyn Any`.
    fn any(&self) -> &dyn Any {
        SizedAny::as_any(self)
    }
    /// Casts to `&mut dyn Any`.
    fn any_mut(&mut self) -> &mut dyn Any {
        SizedAny::as_any_mut(self)
    }
    /// Deletes an entity from this storage.
    #[inline]
    fn delete(&mut self, _entity: EntityId) {}
    /// Deletes all components of this storage.
    #[inline]
    fn clear(&mut self) {}
    /// Returns how much memory this storage uses.
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        None
    }
    /// Returns the storage's name.
    fn name(&self) -> Cow<'static, str> {
        core::any::type_name::<Self>().into()
    }
}
