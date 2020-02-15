use core::any::type_name;
use shipyard::error;
use shipyard::prelude::*;

#[test]
fn returned() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new()) };

    let _view: ViewMut<'static, usize> =
        unsafe { WORLD.as_ref().unwrap() }.run::<&mut usize, _, _>(|usizes| usizes);
    assert_eq!(
        unsafe { WORLD.as_ref().unwrap() }
            .try_run::<&mut usize, _, _>(|usizes| usizes)
            .err(),
        Some(error::GetStorage::StorageBorrow((
            type_name::<usize>(),
            error::Borrow::Unique
        )))
    );
}

#[test]
fn taken_from_run() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new()) };

    let mut view = None;
    unsafe { WORLD.as_ref().unwrap() }.run::<&mut usize, _, _>(|usizes| view = Some(usizes));
    let mut view = None;
    assert_eq!(
        unsafe { WORLD.as_ref().unwrap() }
            .try_run::<&mut usize, _, _>(|usizes| view = Some(usizes))
            .err(),
        Some(error::GetStorage::StorageBorrow((
            type_name::<usize>(),
            error::Borrow::Unique
        )))
    );
}
