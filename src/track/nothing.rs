use crate::seal::Sealed;
use crate::track::{Untracked, UntrackedConst};
use crate::tracking::{Track, Tracking};

impl Sealed for Track<Untracked> {}

impl Tracking for Track<Untracked> {
    fn as_const() -> u32 {
        UntrackedConst
    }
}
