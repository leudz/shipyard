use shipyard::prelude::*;

#[system(RefMut)]
fn run(ref mut usizes: &mut usize) {
    let _result: Result<&mut usize, _> = usizes.get(EntityId::dead());
}

#[system(Ref)]
fn run(ref usizes: &usize, ref u32s: &u32) {
    (usizes, u32s).get(EntityId::dead()).unwrap();
    (usizes, u32s).get(EntityId::dead()).unwrap();
    (usizes, u32s).get(EntityId::dead()).unwrap();
    (usizes, u32s).get(EntityId::dead()).unwrap();
}
