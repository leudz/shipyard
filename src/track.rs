mod all;
mod deletion;
mod deletion_removal;
mod insertion;
mod insertion_deletion;
mod insertion_deletion_removal;
mod insertion_modification;
mod insertion_modification_deletion;
mod insertion_modification_removal;
mod insertion_removal;
mod modification;
mod modification_deletion;
mod modification_deletion_removal;
mod modification_removal;
mod nothing;
mod removal;

#[allow(missing_docs, non_upper_case_globals)]
pub const Untracked: u32 = 0b0000;
#[allow(missing_docs, non_upper_case_globals)]
pub const Insertion: u32 = 0b0001;
#[allow(missing_docs, non_upper_case_globals)]
pub const Modification: u32 = 0b0010;
#[allow(missing_docs, non_upper_case_globals)]
pub const Deletion: u32 = 0b0100;
#[allow(missing_docs, non_upper_case_globals)]
pub const Removal: u32 = 0b1000;
#[allow(missing_docs, non_upper_case_globals)]
pub const All: u32 = Insertion + Modification + Deletion + Removal;
