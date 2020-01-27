use shipyard::prelude::*;

struct NonSendSyncStruct(core::marker::PhantomData<*const ()>);

#[system(NonSendSyncSys)]
fn run(_: NonSendSync<&NonSendSyncStruct>, _: NonSendSync<&mut NonSendSyncStruct>, _: Unique<NonSendSync<&NonSendSyncStruct>>, _: Unique<NonSendSync<&mut NonSendSyncStruct>>) {}

fn main() {}
