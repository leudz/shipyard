use crate::entities::Entities;
use crate::entity_id::EntityId;
use crate::sparse_set::BUCKET_SIZE;
use alloc::vec::Vec;

#[derive(Default)]
pub(super) struct PendingPageAccumulator {
    pub(super) masks: Vec<u32>,
    pub(super) active_pages: Vec<usize>,
}

impl PendingPageAccumulator {
    #[inline]
    pub(super) fn union_page(&mut self, page_index: usize, mask: u32) {
        if mask == 0 {
            return;
        }

        if page_index >= self.masks.len() {
            self.masks.resize(page_index + 1, 0);
        }

        let aggregate = &mut self.masks[page_index];
        if *aggregate == 0 {
            self.active_pages.push(page_index);
        }
        *aggregate |= mask;
    }

    pub(super) fn append_live_entities(&mut self, entities: &Entities, out: &mut Vec<EntityId>) {
        debug_assert_eq!(BUCKET_SIZE, u32::BITS as usize);

        self.active_pages.sort_unstable();

        for page_index in self.active_pages.drain(..) {
            let mut mask = core::mem::take(&mut self.masks[page_index]);

            while mask != 0 {
                let bit_index = mask.trailing_zeros() as usize;
                mask &= mask - 1;

                let entity_index = page_index * BUCKET_SIZE + bit_index;
                if let Some(&entity) = entities.data.get(entity_index) {
                    if entity.uindex() == entity_index {
                        out.push(entity);
                    }
                }
            }
        }
    }
}
