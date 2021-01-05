use crate::components::*;
use crate::geometry::*;
use crate::hud::Hud;
use crate::renderer::SceneRenderer;
use shipyard::*;

pub fn init_world(img_area: Area, stage_area: Area, hud: Hud, renderer: SceneRenderer) -> World {
    let world = World::default();

    world.add_unique(ImageArea(img_area)).unwrap();
    world.add_unique(StageArea(stage_area)).unwrap();
    world.add_unique(InstancePositions(Vec::new())).unwrap();
    world.add_unique(Fps(0)).unwrap();
    world.add_unique(Controller::Waiting).unwrap();
    world.add_unique(FpsCounter::new()).unwrap();
    world.add_unique(Timestamp(0.0)).unwrap();
    world.add_unique_non_send_sync(renderer).unwrap();
    world.add_unique_non_send_sync(hud).unwrap();

    world
}
