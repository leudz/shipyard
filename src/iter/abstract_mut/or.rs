use super::AbstractMut;
use crate::entity_id::EntityId;
use crate::or::{OneOfTwo, Or};

impl<'w, T: AbstractMut, U: AbstractMut> AbstractMut for Or<(T, U)>
where
    <U as AbstractMut>::Index: Into<usize> + Clone,
{
    type Out = OneOfTwo<T::Out, U::Out>;
    type Index = OneOfTwo<T::Index, U::Index>;

    #[inline]
    unsafe fn get_data(&self, _: usize) -> Self::Out {
        unreachable!()
    }
    #[inline]
    unsafe fn get_datas(&self, index: Self::Index) -> Self::Out {
        match index {
            OneOfTwo::One(index) => OneOfTwo::One((self.0).0.get_datas(index)),
            OneOfTwo::Two(index) => OneOfTwo::Two((self.0).1.get_datas(index)),
        }
    }
    #[inline]
    fn indices_of(&self, entity: EntityId, index: usize, mask: u16) -> Option<Self::Index> {
        if index < (self.0).0.len() {
            let index = (self.0).0.indices_of(entity, index, mask)?;
            Some(OneOfTwo::One(index))
        } else {
            let index = (self.0).1.indices_of(entity, index, mask)?;
            if (self.0)
                .0
                .indices_of(entity, index.clone().into(), mask)
                .is_some()
            {
                return None;
            }
            Some(OneOfTwo::Two(index))
        }
    }
    #[inline]
    #[allow(clippy::manual_map)]
    fn indices_of_passenger(
        &self,
        entity: EntityId,
        index: usize,
        mask: u16,
    ) -> Option<Self::Index> {
        if let Some(index) = (self.0).0.indices_of(entity, index, mask) {
            Some(OneOfTwo::One(index))
        } else if let Some(index) = (self.0).1.indices_of(entity, index, mask) {
            Some(OneOfTwo::Two(index))
        } else {
            None
        }
    }
    #[inline]
    unsafe fn indices_of_unchecked(&self, _: EntityId, _: usize, _: u16) -> Self::Index {
        unreachable!()
    }
    #[inline]
    unsafe fn get_id(&self, _: usize) -> EntityId {
        unreachable!()
    }
    fn len(&self) -> usize {
        0
    }
}

// impl<'w, T: Component> AbstractMut for Not<FullRawWindowMut<'w, T, T::Tracking>> {
//     type Out = ();
//     type Index = usize;

//     #[inline]
//     unsafe fn get_data(&self, _: usize) -> Self::Out {}
//     #[inline]
//     unsafe fn get_datas(&self, _: Self::Index) -> Self::Out {}
//     #[inline]
//     fn indices_of(&self, entity: EntityId, _: usize, _: u16) -> Option<Self::Index> {
//         if self.0.index_of(entity).is_some() {
//             None
//         } else {
//             Some(core::usize::MAX)
//         }
//     }
//     #[inline]
//     unsafe fn indices_of_unchecked(&self, _: EntityId, _: usize, _: u16) -> Self::Index {
//         unreachable!()
//     }
//     #[inline]
//     unsafe fn get_id(&self, _: usize) -> EntityId {
//         unreachable!()
//     }
// }
