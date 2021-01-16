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

pub trait UnknownStorage: SizedAny {
    fn any(&self) -> &dyn Any {
        SizedAny::as_any(self)
    }
    fn any_mut(&mut self) -> &mut dyn Any {
        SizedAny::as_any_mut(self)
    }
    #[inline]
    fn delete(&mut self, _entity: EntityId) {}
    #[inline]
    fn clear(&mut self) {}
    fn memory_usage(&self) -> Option<StorageMemoryUsage> {
        None
    }
    fn name(&self) -> Cow<'static, str> {
        core::any::type_name::<Self>().into()
    }
}
