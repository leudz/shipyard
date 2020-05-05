use shipyard::*;

#[allow(unused)]
fn ref_mut(ref mut u32s: ViewMut<u32>) {
    let id = EntityId::dead();
    let i: &mut u32 = u32s.get(id).unwrap();
}

#[allow(unused)]
fn ref_sys(ref u32s: View<u32>, ref usizes: View<usize>) {
    let id = EntityId::dead();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
    (usizes, u32s).get(id).unwrap();
}
