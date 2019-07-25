use super::Key;

/// View into the entities, this allows to add and remove entities.
pub struct EntityViewMut<'a> {
    pub(super) data: &'a mut Vec<Key>,
    pub(crate) list: &'a mut Option<(usize, usize)>,
}
