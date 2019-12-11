use crate::sparse_set::{View, ViewMut};
use crate::storage::Key;

/// Retrives components based on their type and entity key.
pub trait GetComponent {
    type Out;
    /// Retrieve components of `entity`.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    /// #Example
    /// ```
    /// # use shipyard::prelude::*;
    /// let world = World::new::<(usize, u32)>();
    ///
    /// world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
    ///     let entity = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    ///     assert_eq!((&usizes, &u32s).get(entity), Some((&0, &1)));
    /// });
    /// ```
    fn get(self, entity: Key) -> Option<Self::Out>;
}

impl<'a: 'b, 'b, T: 'static> GetComponent for &'b View<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get(entity)
    }
}

impl<'a: 'b, 'b, T: 'static> GetComponent for &'b ViewMut<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get(entity)
    }
}

impl<'a: 'b, 'b, T: 'static> GetComponent for &'b mut ViewMut<'a, T> {
    type Out = &'b mut T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get_mut(entity)
    }
}

macro_rules! impl_get_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: GetComponent),+> GetComponent for ($($type,)+) {
            type Out = ($($type::Out,)+);
            fn get(self, entity: Key) -> Option<Self::Out> {
                Some(($(self.$index.get(entity)?,)+))
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
