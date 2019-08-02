use std::any::TypeId;
use std::sync::Arc;

pub(super) struct PackInfo {
    pub(super) owned_type: Arc<[TypeId]>,
    pub(super) owned_len: usize,
}

impl Default for PackInfo {
    fn default() -> Self {
        PackInfo {
            owned_type: Arc::new([]),
            owned_len: 0,
        }
    }
}
