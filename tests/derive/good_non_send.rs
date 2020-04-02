use shipyard::prelude::*;

struct NonSendStruct(core::marker::PhantomData<*const ()>);

unsafe impl Sync for NonSendStruct {}

#[system(NonSendSys)]
fn run(_: NonSend<&NonSendStruct>, _: Unique<NonSend<&NonSendStruct>>) {}

#[system(MutNonSendSys)]
fn run(_: NonSend<&mut NonSendStruct>) {}

#[system(UniqueMutNonSendSys)]
fn run(_: Unique<NonSend<&mut NonSendStruct>>) {}

fn main() {}
