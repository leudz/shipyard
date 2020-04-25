use crate::error;
use crate::sparse_set::{Window, WindowMut};
use crate::storage::EntityId;
use crate::view::{View, ViewMut};
use core::any::type_name;

/// Retrives components based on their type and entity id.
pub trait Get {
    type Out;
    /// Retrieve components of `entity`.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, Get, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         let entity = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         assert_eq!((&usizes, &u32s).try_get(entity), Ok((&0, &1)));
    ///     },
    /// );
    /// ```
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent>;
    /// Retrieve components of `entity`.  
    /// Unwraps errors.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{EntitiesViewMut, Get, ViewMut, World};
    ///
    /// let world = World::new();
    ///
    /// world.run(
    ///     |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
    ///         let entity = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///         assert_eq!((&usizes, &u32s).get(entity), (&0, &1));
    ///     },
    /// );
    /// ```
    fn get(self, entity: EntityId) -> Self::Out;
}

impl<'a: 'b, 'b, T: 'static> Get for &'b Window<'a, T> {
    type Out = &'b T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        self.get(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

impl<'a: 'b, 'b, T: 'static> Get for &'b WindowMut<'a, T> {
    type Out = &'b T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        self.get(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

impl<'a: 'b, 'b, T: 'static> Get for &'b mut WindowMut<'a, T> {
    type Out = &'b mut T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        self.get_mut(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

impl<'a: 'b, 'b, T: 'static> Get for &'b View<'a, T> {
    type Out = &'b T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        (**self).get(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

impl<'a: 'b, 'b, T: 'static> Get for &'b ViewMut<'a, T> {
    type Out = &'b T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        (**self).get(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

impl<'a: 'b, 'b, T: 'static> Get for &'b mut ViewMut<'a, T> {
    type Out = &'b mut T;
    fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        self.get_mut(entity).ok_or_else(|| error::MissingComponent {
            id: entity,
            name: type_name::<T>(),
        })
    }
    fn get(self, entity: EntityId) -> Self::Out {
        self.try_get(entity).unwrap()
    }
}

macro_rules! impl_get_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Get),+> Get for ($($type,)+) {
            type Out = ($($type::Out,)+);
            fn try_get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
                Ok(($(self.$index.try_get(entity)?,)+))
            }
            fn get(self, entity: EntityId) -> Self::Out {
                self.try_get(entity).unwrap()
            }
        }
    }
}

macro_rules! get_component {
    ($(($type: ident, $index: tt))+; ($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_get_component![$(($type, $index))*];
        get_component![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_get_component![$(($type, $index))*];
    }
}

get_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
