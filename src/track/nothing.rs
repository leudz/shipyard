use crate::seal::Sealed;
use crate::track::Untracked;
use crate::tracking::Tracking;

impl Sealed for Untracked {}
impl Tracking for Untracked {
    const VALUE: u32 = 0;

    fn name() -> &'static str {
        unreachable!()
    }
}
