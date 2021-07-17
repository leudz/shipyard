use crate::geometry::*;
//re-exported so its easier to just use components::*
pub use crate::fps::FpsCounter;
pub use crate::hud::Hud;
pub use crate::renderer::SceneRenderer;
use shipyard::Component;

#[derive(Component)]
pub struct ImageArea(pub Area);

#[derive(Component)]
pub struct StageArea(pub Area);

#[derive(Component)]
pub struct InstancePositions(pub Vec<f32>);

#[derive(Component)]
pub struct Fps(pub u32);

#[derive(Component)]
pub struct Timestamp(pub f64);

#[derive(Component, PartialEq)]
pub enum Controller {
    Adding,
    Waiting,
}

//the bunnies
#[derive(Component)]
pub struct Position(pub Point);

#[derive(Component)]
pub struct Speed(pub Point);

#[derive(Component)]
pub struct Gravity(pub f64);
