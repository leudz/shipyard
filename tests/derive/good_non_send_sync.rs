use shipyard::prelude::*;

struct NonSendSyncStruct(core::marker::PhantomData<*const ()>);

#[system(NonSendSyncSys)]
fn run(_: NonSendSync<&NonSendSyncStruct>, _: Unique<NonSendSync<&NonSendSyncStruct>>) {}

#[system(MutNonSendSyncSys)]
fn run(_: NonSendSync<&mut NonSendSyncStruct>) {}

#[system(UniqueMutNonSendSyncSys)]
fn run(_: Unique<NonSendSync<&mut NonSendSyncStruct>>) {}

fn main() {}
