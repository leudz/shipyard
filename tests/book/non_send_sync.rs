use shipyard::*;
use std::rc::Rc;

// ANCHOR: non_send_sync
#[derive(Unique)]
struct RcU64(Rc<u64>);
#[derive(Component)]
struct RcUSIZE(Rc<usize>);

#[allow(unused)]
fn run(rcs_usize: NonSendSync<View<RcUSIZE>>, rc_u64: NonSendSync<UniqueView<RcU64>>) {}
// ANCHOR_END: non_send_sync

#[test]
#[should_panic]
fn test() {
    World::new().run(run);
}
