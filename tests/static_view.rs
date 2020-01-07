use shipyard::prelude::*;

#[test]
#[should_panic]
fn returned() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new::<(usize,)>()) };

    let _view: shipyard::internal::ViewMut<'static, usize> =
        unsafe { WORLD.as_ref().unwrap() }.run::<&mut usize, _, _>(|usizes| usizes);
    let _view: shipyard::internal::ViewMut<'static, usize> =
        unsafe { WORLD.as_ref().unwrap() }.run::<&mut usize, _, _>(|usizes| usizes);
}
