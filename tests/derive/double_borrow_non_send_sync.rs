use shipyard::prelude::*;

struct NonSendSyncStruct(core::marker::PhantomData<*const ()>);

#[system(NonSendSyncSys)]
fn run(_: NonSendSync<&NonSendSyncStruct>, _: NonSendSync<&mut NonSendSyncStruct>) {}

#[system(NonSendSyncSysUnique)]
fn run(_: NonSendSync<&NonSendSyncStruct>, _: Unique<NonSendSync<&mut NonSendSyncStruct>>) {}

#[system(UniqueNonSendSyncSys)]
fn run(_: Unique<NonSendSync<&NonSendSyncStruct>>, _: NonSendSync<&mut NonSendSyncStruct>) {}

#[system(UniqueUnique)]
fn run(_: Unique<NonSendSync<&NonSendSyncStruct>>, _: Unique<NonSendSync<&mut NonSendSyncStruct>>) {}

fn main() {}
