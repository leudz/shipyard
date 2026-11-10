use shipyard::scheduler::IntoWorkloadSystem;
use shipyard::*;

#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

fn two_views(_: View<U32>, _: ViewMut<U32>) {}
fn two_views_mut(_: ViewMut<U32>, _: ViewMut<U32>) {}
fn two_views_mut_mid(_: ViewMut<U32>, _: ViewMut<U32>, _: EntitiesView, _: EntitiesView) {}
fn all_storages(_: AllStoragesViewMut, _: EntitiesView) {}

#[test]
fn bad_systems() {
    assert_eq!(
        two_views.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViews)
    );
    assert_eq!(
        two_views_mut.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        two_views_mut_mid.into_workload_system().err(),
        Some(error::InvalidSystem::MultipleViewsMut)
    );
    assert_eq!(
        all_storages.into_workload_system().err(),
        Some(error::InvalidSystem::AllStorages)
    );
}

/// Always returns a `SystemError`.
fn fails() -> Result<(), error::SystemError> {
    Err("fails".into())
}

/// Returns a `SystemError` while borrowing `U32`.
fn fails_with_view(_: View<U32>) -> Result<(), error::SystemError> {
    Err("fails_with_view".into())
}

/// Asserts that `World::run` returns a fallible system's error as is, with and without views.
#[test]
fn world_run_fallible() {
    let world = World::new();

    let without_views = world.run(fails).unwrap_err().to_string();
    let with_view = world.run(fails_with_view).unwrap_err().to_string();

    assert_eq!(
        (without_views.as_str(), with_view.as_str()),
        ("fails", "fails_with_view")
    );
}

/// Asserts that `World::run` returns any other value as is, including other results.
#[test]
fn world_run_other() {
    let world = World::new();

    let value = world.run(|| 4u32);
    let result = world.run(|| -> Result<u32, error::SystemError> { Ok(5) });

    assert_eq!((value, result.ok()), (4, Some(5)));
}

/// Asserts that `World::run_with_data` returns a fallible system's error as is.
#[test]
fn world_run_with_data_fallible() {
    let world = World::new();

    let result = world.run_with_data(
        |message: &'static str| -> Result<(), error::SystemError> { Err(message.into()) },
        "data",
    );

    assert_eq!(result.unwrap_err().to_string(), "data");
}

/// Asserts that `AllStorages::run` returns a fallible system's error as is, with and without views.
#[test]
fn all_storages_run_fallible() {
    let world = World::new();
    let all_storages = world.all_storages().unwrap();

    let without_views = all_storages.run(fails).unwrap_err().to_string();
    let with_view = all_storages.run(fails_with_view).unwrap_err().to_string();

    assert_eq!(
        (without_views.as_str(), with_view.as_str()),
        ("fails", "fails_with_view")
    );
}

/// Asserts that `AllStorages::run_with_data` returns any value as is.
#[test]
fn all_storages_run_with_data() {
    let world = World::new();
    let all_storages = world.all_storages().unwrap();

    let value = all_storages.run_with_data(|data: u32| data + 1, 4);
    let result = all_storages.run_with_data(
        |message: &'static str| -> Result<(), error::SystemError> { Err(message.into()) },
        "data",
    );

    assert_eq!(
        (value, result.unwrap_err().to_string().as_str()),
        (5, "data")
    );
}
