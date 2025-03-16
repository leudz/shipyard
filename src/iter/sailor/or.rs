use crate::entity_id::EntityId;
use crate::iter::{ShiperatorCaptain, ShiperatorSailor};
use crate::or::{OneOfTwo, OrWindow};

impl<T: ShiperatorCaptain + ShiperatorSailor, U: ShiperatorCaptain + ShiperatorSailor>
    ShiperatorSailor for OrWindow<(T, U)>
{
    type Index = OneOfTwo<T::Index, U::Index>;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        match index {
            OneOfTwo::One(index) => OneOfTwo::One((self.storages).0.get_sailor_data(index)),
            OneOfTwo::Two(index) => OneOfTwo::Two((self.storages).1.get_sailor_data(index)),
        }
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, index: usize) -> Option<Self::Index> {
        if self.is_captain {
            if self.is_past_first_storage {
                let Some(index) = (self.storages).1.indices_of(eid, index) else {
                    return None;
                };

                if (self.storages).0.indices_of(eid, 0).is_some() {
                    return None;
                }

                Some(OneOfTwo::Two(index))
            } else {
                let Some(index) = (self.storages).0.indices_of(eid, index) else {
                    return None;
                };

                Some(OneOfTwo::One(index))
            }
        } else {
            let index0 = (self.storages).0.indices_of(eid, index);
            let index1 = (self.storages).1.indices_of(eid, index);

            match (index0, index1) {
                (None, None) => None,
                (None, Some(index1)) => Some(OneOfTwo::Two(index1)),
                (Some(index0), None) => Some(OneOfTwo::One(index0)),
                (Some(index0), Some(_)) => Some(OneOfTwo::One(index0)),
            }
        }
    }

    #[inline]
    fn index_from_usize(_index: usize) -> Self::Index {
        unreachable!()
    }
}
