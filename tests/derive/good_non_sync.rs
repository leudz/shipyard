use shipyard::prelude::*;

struct NonSyncStruct(core::marker::PhantomData<*const ()>);

unsafe impl Send for NonSyncStruct {}

#[system(NonSyncSys)]
fn run(_: NonSync<&NonSyncStruct>, _: Unique<NonSync<&NonSyncStruct>>) {}

#[system(MutNonSyncSys)]
fn run(_: NonSync<&mut NonSyncStruct>) {}

#[system(UniqueNonSyncSys)]
fn run(_: Unique<NonSync<&mut NonSyncStruct>>) {}

fn main() {}
