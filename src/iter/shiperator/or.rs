use crate::entity_id::EntityId;
use crate::iter::Shiperator;
use crate::or::{OneOfTwo, OrWindow};
use crate::sparse_set::RawEntityIdAccess;

impl<T: Shiperator, U: Shiperator> Shiperator for OrWindow<(T, U)> {
    type Out = OneOfTwo<T::Out, U::Out>;
    type Index = OneOfTwo<T::Index, U::Index>;

    #[inline]
    fn next_slice(&mut self) {
        self.is_past_first_storage = true;
    }

    #[inline]
    fn sail_time(&self) -> usize {
        (self.storages).0.sail_time() + (self.storages).1.sail_time()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        entities: &mut RawEntityIdAccess,
        current: usize,
    ) -> Option<Self::Index> {
        if self.is_past_first_storage {
            let Some(index) = (self.storages).1.captain_indices_of(entities, current) else {
                return None;
            };

            let eid = entities.get(current);

            if (self.storages).0.sailor_indices_of(eid).is_some() {
                return None;
            }

            Some(OneOfTwo::Two(index))
        } else {
            let Some(index) = (self.storages).0.captain_indices_of(entities, current) else {
                return None;
            };

            Some(OneOfTwo::One(index))
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        // Option::map can throw the compiler off
        // I prefer taking the readability hit
        #[allow(clippy::manual_map)]
        if let Some(index) = (self.storages).0.sailor_indices_of(eid) {
            Some(OneOfTwo::One(index))
        } else if let Some(index) = (self.storages).1.sailor_indices_of(eid) {
            Some(OneOfTwo::Two(index))
        } else {
            None
        }
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        match index {
            OneOfTwo::One(index) => OneOfTwo::One((self.storages).0.get_data(index)),
            OneOfTwo::Two(index) => OneOfTwo::Two((self.storages).1.get_data(index)),
        }
    }
}
