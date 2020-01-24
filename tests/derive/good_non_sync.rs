use shipyard::prelude::*;

struct NonSyncStruct(core::marker::PhantomData<*const ()>);

unsafe impl Send for NonSyncStruct {}

#[system(NonSyncSys)]
fn run(_: NonSync<&NonSyncStruct>, _: NonSync<&mut NonSyncStruct>, _: Not<NonSync<&NonSyncStruct>>, _: Not<NonSync<&mut NonSyncStruct>>, _: Unique<NonSync<&NonSyncStruct>>, _: Unique<NonSync<&mut NonSyncStruct>>) {}

fn main() {}
