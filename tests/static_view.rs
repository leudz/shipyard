use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn returned() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new()) };

    let _view: ViewMut<'static, usize> =
        unsafe { WORLD.as_ref().unwrap() }.run(|usizes: ViewMut<usize>| usizes);
    match unsafe { WORLD.as_ref().unwrap() }
        .try_run(|usizes: ViewMut<usize>| usizes)
        .err()
    {
        Some(error::Run::GetStorage(get_storage)) => {
            assert_eq!(
                get_storage,
                error::GetStorage::StorageBorrow((type_name::<usize>(), error::Borrow::Unique))
            );
        }
        _ => panic!(),
    }
}

#[test]
fn taken_from_run() {
    static mut WORLD: Option<World> = None;

    unsafe { WORLD = Some(World::new()) };

    let mut view = None;
    unsafe { WORLD.as_ref().unwrap() }.run(|usizes: ViewMut<usize>| view = Some(usizes));
    let mut view = None;
    match unsafe { WORLD.as_ref().unwrap() }
        .try_run(|usizes: ViewMut<usize>| view = Some(usizes))
        .err()
    {
        Some(error::Run::GetStorage(get_storage)) => {
            assert_eq!(
                get_storage,
                error::GetStorage::StorageBorrow((type_name::<usize>(), error::Borrow::Unique))
            );
        }
        _ => panic!(),
    }
}
