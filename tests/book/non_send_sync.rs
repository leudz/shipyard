use shipyard::borrow::NonSendSync;
use shipyard::*;
use std::rc::Rc;

// ANCHOR: non_send_sync
#[derive(Unique)]
struct RcU32(Rc<u32>);
#[derive(Component)]
struct RcUSIZE(Rc<usize>);

#[allow(unused)]
fn run(rcs_usize: NonSendSync<View<RcUSIZE>>, rc_u32: NonSendSync<UniqueView<RcU32>>) {}
// ANCHOR_END: non_send_sync

#[test]
#[should_panic]
fn test() {
    World::new().run(run);
}
