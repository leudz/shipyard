use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::r#mut::Mut;
use crate::sparse_set::SparseSet;
use crate::view::{View, ViewMut};
use core::any::type_name;

/// Retrieves components based on their type and entity id.
pub trait Get {
    #[allow(missing_docs)]
    type Out;
    /// Retrieve components of `entity`.
    ///
    /// Multiple components can be queried at the same time using a tuple.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, Get, View, World};
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct U32(u32);
    ///
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct USIZE(usize);
    ///
    /// let mut world = World::new();
    ///
    /// let entity = world.add_entity((USIZE(0), U32(1)));
    ///
    /// let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
    /// assert_eq!((&usizes, &u32s).get(entity), Ok((&USIZE(0), &U32(1))));
    /// ```
    fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent>;
}

impl<'a, 'b, T: Component, const TRACK: u32> Get for &'b View<'a, T, TRACK> {
    type Out = &'b T;

    #[inline]
    fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        (**self)
            .private_get(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })
    }
}

impl<'a, 'b, T: Component, const TRACK: u32> Get for &'b ViewMut<'a, T, TRACK> {
    type Out = &'b T;

    #[inline]
    fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        (**self)
            .private_get(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })
    }
}

impl<'a, 'b, T: Component, const TRACK: u32> Get for &'b mut ViewMut<'a, T, TRACK> {
    type Out = Mut<'b, T>;

    #[inline]
    fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        let index = self
            .index_of(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })?;

        let SparseSet {
            data,
            modification_data,
            is_tracking_modification,
            ..
        } = self.sparse_set;

        Ok(Mut {
            flag: is_tracking_modification
                .then(|| unsafe { modification_data.get_unchecked_mut(index) }),
            current: self.current,
            data: unsafe { data.get_unchecked_mut(index) },
        })
    }
}

macro_rules! impl_get_component {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: Get),+> Get for ($($type,)+) {
            type Out = ($($type::Out,)+);
            #[inline]
            fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
                Ok(($(self.$index.get(entity)?,)+))
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
