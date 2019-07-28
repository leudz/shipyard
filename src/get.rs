use crate::entity::Key;
use crate::not::Not;
use crate::sparse_array::{Read, Write};

pub trait GetComponent {
    type Out;
    /// Retrieve components of `entity`.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    fn get(self, entity: Key) -> Option<Self::Out>;
}

impl<'a, T> GetComponent for Read<'a, T> {
    type Out = &'a T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get(entity.index())
    }
}

impl<'a, 'b, T> GetComponent for &'b Read<'a, T> {
    type Out = &'b T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get(entity.index())
    }
}

impl<'a, T> GetComponent for Write<'a, T> {
    type Out = &'a mut T;
    fn get(self, entity: Key) -> Option<Self::Out> {
        self.inner.get_mut(entity.index())
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

impl<'a, T> GetComponent for Not<Read<'a, T>> {
    type Out = ();
    fn get(self, entity: Key) -> Option<Self::Out> {
        if self.0.contains(entity.index()) {
            None
        } else {
            Some(())
        }
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

impl<'a, T> GetComponent for Not<Write<'a, T>> {
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

get_component![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
