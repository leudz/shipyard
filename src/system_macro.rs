#[macro_export]
macro_rules! system {
    ($function: expr) => {{
        (
            |world: &shipyard::World| world.try_run($function).map(drop),
            $function,
        )
    }};
}

#[macro_export]
macro_rules! try_system {
    ($function: expr) => {{
        (
            |world: &shipyard::World| {
                world
                    .try_run($function)?
                    .map_err(shipyard::error::Run::from_custom)
            },
            $function,
        )
    }};
}
