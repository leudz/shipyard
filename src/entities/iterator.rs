use super::Entities;
use crate::entity_id::EntityId;

#[allow(clippy::type_complexity)]
pub struct EntitiesIter<'a>(
    core::iter::FilterMap<
        core::iter::Enumerate<core::slice::Iter<'a, EntityId>>,
        fn((usize, &EntityId)) -> Option<EntityId>,
    >,
);

impl<'a> IntoIterator for &'a Entities {
    type Item = EntityId;
    type IntoIter = EntitiesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        EntitiesIter(self.data.iter().enumerate().filter_map(filter_map))
    }
}

fn filter_map((i, &entity): (usize, &EntityId)) -> Option<EntityId> {
    if i == entity.uindex() {
        Some(entity)
    } else {
        None
    }
}

impl<'a> Iterator for EntitiesIter<'a> {
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
