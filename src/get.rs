use crate::component::Component;
use crate::entity_id::EntityId;
use crate::error;
use crate::r#mut::Mut;
use crate::sparse_set::SparseSet;
use crate::tracking::Tracking;
use crate::views::{View, ViewMut};
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

impl<'a, T: Component> Get for &'a SparseSet<T> {
    type Out = &'a T;

    fn get(self, entity: EntityId) -> Result<Self::Out, error::MissingComponent> {
        self.private_get(entity)
            .ok_or_else(|| error::MissingComponent {
                id: entity,
                name: type_name::<T>(),
            })
    }
}

impl<'a, 'b, T: Component, Track: Tracking> Get for &'b View<'a, T, Track> {
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

impl<'a, 'b, T: Component, Track: Tracking> Get for &'b ViewMut<'a, T, Track> {
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

impl<'a, 'b, T: Component, Track: Tracking> Get for &'b mut ViewMut<'a, T, Track> {
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

#[cfg(not(feature = "extended_tuple"))]
get_component![(ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)];
#[cfg(feature = "extended_tuple")]
get_component![
    (ViewA, 0); (ViewB, 1) (ViewC, 2) (ViewD, 3) (ViewE, 4) (ViewF, 5) (ViewG, 6) (ViewH, 7) (ViewI, 8) (ViewJ, 9)
    (ViewK, 10) (ViewL, 11) (ViewM, 12) (ViewN, 13) (ViewO, 14) (ViewP, 15) (ViewQ, 16) (ViewR, 17) (ViewS, 18) (ViewT, 19)
    (ViewU, 20) (ViewV, 21) (ViewW, 22) (ViewX, 23) (ViewY, 24) (ViewZ, 25) (ViewAA, 26) (ViewBB, 27) (ViewCC, 28) (ViewDD, 29)
    (ViewEE, 30) (ViewFF, 31)
];
