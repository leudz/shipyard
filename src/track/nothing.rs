use crate::seal::Sealed;
use crate::track::Untracked;
use crate::tracking::{Track, Tracking};

impl Sealed for Track<Untracked> {}

impl Tracking for Track<Untracked> {}
