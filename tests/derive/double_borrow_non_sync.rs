use shipyard::prelude::*;

struct NonSyncStruct(core::marker::PhantomData<*const ()>);

unsafe impl Send for NonSyncStruct {}

#[system(NonSyncSys)]
fn run(_: NonSync<&NonSyncStruct>, _: NonSync<&mut NonSyncStruct>) {}

#[system(NonSyncSysUnique)]
fn run(_: NonSync<&NonSyncStruct>, _: Unique<NonSync<&mut NonSyncStruct>>) {}

#[system(UniqueNonSyncSys)]
fn run(_: Unique<NonSync<&NonSyncStruct>>, _: NonSync<&mut NonSyncStruct>) {}

#[system(UniqueUnique)]
fn run(_: Unique<NonSync<&NonSyncStruct>>, _: Unique<NonSync<&mut NonSyncStruct>>) {}

fn main() {}
