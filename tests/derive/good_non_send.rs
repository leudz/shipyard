use shipyard::prelude::*;

struct NonSendStruct(core::marker::PhantomData<*const ()>);

unsafe impl Sync for NonSendStruct {}

#[system(NonSendSys)]
fn run(_: NonSend<&NonSendStruct>, _: NonSend<&mut NonSendStruct>, _: Not<NonSend<&NonSendStruct>>, _: Not<NonSend<&mut NonSendStruct>>) {}

fn main() {}
