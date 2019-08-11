use crate::entity::Key;
use crate::not::Not;
use crate::sparse_array::{Read, View, ViewMut, Write};

/// Retrives components based on their type and entity key.
pub trait GetComponent {
    type Out;
    /// Retrieve components of `entity`.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    /// #Example
    /// ```
    /// # use shipyard::*;
    /// let world = World::new::<(usize, u32)>();
    /// let entity = world.new_entity((0usize, 1u32));
    ///
    /// let (usizes, u32s) = world.get_storage::<(&usize, &u32)>();
    /// assert_eq!((&usizes, &u32s).get(entity), Some((&0, &1)));
    /// ```
    fn get(self, entity: Key) -> Option<Self::Out>;
}

impl<'a: 'b, 'b, T> GetComponent for &'b View<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get(entity.index())
    }
}

impl<'a: 'b, 'b, T> GetComponent for &'b ViewMut<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get(entity.index())
    }
}

impl<'a: 'b, 'b, T> GetComponent for &'b mut ViewMut<'a, T> {
    type Out = &'b mut T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.get_mut(entity.index())
    }
}

impl<'a, 'b, T> GetComponent for &'b Not<View<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b View<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for &'b Not<ViewMut<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b ViewMut<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for &'b mut Not<ViewMut<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b mut ViewMut<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains_index(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a: 'b, 'b, T> GetComponent for &'b Read<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get(entity.index())
    }
}

impl<'a, 'b, T> GetComponent for &'b Write<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get(entity.index())
    }
}

impl<'a, 'b, T> GetComponent for &'b mut Write<'a, T> {
    type Out = &'b mut T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get_mut(entity.index())
    }
}

impl<'a, 'b, T> GetComponent for &'b Not<Read<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b Read<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for &'b Not<Write<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b Write<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for &'b mut Not<Write<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
    }
}

impl<'a, 'b, T> GetComponent for Not<&'b mut Write<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
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
    ($(($left_type: ident, $left_index: tt))+; ($type1: ident, $index1: tt) $(($type: ident, $index: tt))*) => {
        impl_get_component![$(($left_type, $left_index))*];
        get_component![$(($left_type, $left_index))* ($type1, $index1); $(($type, $index))*];
    };
    ($(($type: ident, $index: tt))+;) => {
        impl_get_component![$(($type, $index))*];
    }
}

get_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
