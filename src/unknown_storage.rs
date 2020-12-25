use crate::all_storages::AllStorages;
use crate::entity_id::EntityId;
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
    #[inline]
    fn has_remove_event_to_dispatch(&self) -> bool {
        false
    }
    #[inline]
    fn run_on_remove_global(&mut self, _all_storages: &AllStorages) {}
}
