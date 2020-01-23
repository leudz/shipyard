use crate::geometry::*;
use std::sync::{Arc, Mutex};
pub use crate::fps::FpsCounter;
use crate::hud::Hud;
use crate::renderer::SceneRenderer;

pub struct ImageArea(pub Area);
pub struct StageArea(pub Area);
pub struct InstancePositions(pub Vec<f32>);
pub struct Fps(pub u32);
pub struct Timestamp(pub f64);
#[derive(PartialEq)]
pub enum Controller {
    Adding,
    Waiting
}

pub struct Renderer(pub Arc<Mutex<SceneRenderer>>);

//the bunnies
pub struct Position(pub Point);
pub struct Speed(pub Point);
pub struct Gravity(pub f64);