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

#[allow(missing_docs)]
pub struct Untracked;
#[allow(missing_docs)]
pub struct Insertion;
#[allow(missing_docs)]
pub struct InsertionAndModification;
#[allow(missing_docs)]
pub struct InsertionAndModificationAndDeletion;
#[allow(missing_docs)]
pub struct InsertionAndModificationAndRemoval;
#[allow(missing_docs)]
pub struct InsertionAndDeletion;
#[allow(missing_docs)]
pub struct InsertionAndRemoval;
#[allow(missing_docs)]
pub struct InsertionAndDeletionAndRemoval;
#[allow(missing_docs)]
pub struct Modification;
#[allow(missing_docs)]
pub struct ModificationAndDeletion;
#[allow(missing_docs)]
pub struct ModificationAndRemoval;
#[allow(missing_docs)]
pub struct ModificationAndDeletionAndRemoval;
#[allow(missing_docs)]
pub struct Deletion;
#[allow(missing_docs)]
pub struct DeletionAndRemoval;
#[allow(missing_docs)]
pub struct Removal;
#[allow(missing_docs)]
pub struct All;

#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const UntrackedConst: u32 = 0b0000;
#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const InsertionConst: u32 = 0b0001;
#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const ModificationConst: u32 = 0b0010;
#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const DeletionConst: u32 = 0b0100;
#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const RemovalConst: u32 = 0b1000;
#[allow(missing_docs, non_upper_case_globals)]
pub(crate) const AllConst: u32 = InsertionConst + ModificationConst + DeletionConst + RemovalConst;
