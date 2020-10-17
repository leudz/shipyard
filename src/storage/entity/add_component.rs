use super::{Entities, EntityId};
use crate::error;
use crate::view::ViewMut;

// No new storage will be created
/// Adds components to an existing entity without creating new storage.
pub trait AddComponent<T> {
    fn try_add_component(
        self,
        component: T,
        entity: EntityId,
        entities: &Entities,
    ) -> Result<(), error::AddComponent>;
    #[cfg(feature = "panic")]
    #[cfg_attr(docsrs, doc(cfg(feature = "panic")))]
    fn add_component(self, component: T, entity: EntityId, entities: &Entities);
}

impl<T: 'static> AddComponent<T> for &mut ViewMut<'_, T> {
    fn try_add_component(
        self,
        component: T,
        entity: EntityId,
        entities: &Entities,
    ) -> Result<(), error::AddComponent> {
        if entities.is_alive(entity) {
            self.insert(component, entity);
            Ok(())
        } else {
            Err(error::AddComponent::EntityIsNotAlive)
        }
    }
    #[cfg(feature = "panic")]
    #[track_caller]
    fn add_component(self, component: T, entity: EntityId, entities: &Entities) {
        match self.try_add_component(component, entity, entities) {
            Ok(_) => (),
            Err(err) => panic!("{:?}", err),
        }
    }
}

macro_rules! impl_add_component {
    // add is short for additional
    ($(($type: ident, $index: tt))+; $(($add_type: ident, $add_index: tt))*) => {
        impl<$($type: 'static,)+ $($add_type: 'static),*> AddComponent<($($type,)+)> for ($(&mut ViewMut<'_, $type>,)+ $(&mut ViewMut<'_, $add_type>,)*) {
            fn try_add_component(self, component: ($($type,)+), entity: EntityId, entities: &Entities) -> Result<(), error::AddComponent> {
                if entities.is_alive(entity) {
                    $(
                        self.$index.insert(component.$index, entity);
                    )+

                    Ok(())
                } else {
                    Err(error::AddComponent::EntityIsNotAlive)
                }
            }
            #[cfg(feature = "panic")]
            #[track_caller]
            fn add_component(self, component: ($($type,)+), entity: EntityId, entities: &Entities) {
                match self.try_add_component(component, entity, entities) {
                    Ok(_) => (),
                    Err(err) => panic!("{:?}", err),
                }
            }
        }
    }
}

macro_rules! add_component {
    (($type1: ident, $index1: tt) $(($type: ident, $index: tt))*;; ($queue_type1: ident, $queue_index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![($type1, $index1) $(($type, $index))*;];
        add_component![($type1, $index1); $(($type, $index))* ($queue_type1, $queue_index1); $(($queue_type, $queue_index))*];
    };
    // add is short for additional
    ($(($type: ident, $index: tt))+; ($add_type1: ident, $add_index1: tt) $(($add_type: ident, $add_index: tt))*; $(($queue_type: ident, $queue_index: tt))*) => {
        impl_add_component![$(($type, $index))+; ($add_type1, $add_index1) $(($add_type, $add_index))*];
        add_component![$(($type, $index))+ ($add_type1, $add_index1); $(($add_type, $add_index))*; $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;;) => {
        impl_add_component![$(($type, $index))+;];
    }
}

add_component![(A, 0);; (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
