use shipyard::prelude::*;
use crate::components::*;
use crate::geometry::*;
use crate::config::*;

pub const TICK:&'static str = "TICK";

pub fn register_workloads(world:&World) {
    world.add_workload::<(Start, HandleController, Update, Render, End), _>(TICK); 
}

#[system(Start)]
pub fn run (mut fps_counter: Unique<&mut FpsCounter>) {
    fps_counter.begin();
}

#[system(HandleController)]
pub fn run (
    mut entities:EntitiesMut, 
    controller: Unique<&Controller>, 
    mut positions: &mut Position, 
    mut speeds:&mut Speed, 
    mut gravities:&mut Gravity, 
    stage_area:Unique<&StageArea>, 
    img_area:Unique<&ImageArea>,
    mut instance_positions:Unique<&mut InstancePositions>
) {
    if *controller == Controller::Adding {
        let mut count = positions.len();
        let len = count + N_BUNNIES_PER_TICK;
        let stage_size = stage_area.0;
        let img_size = img_area.0;


        for count in 0..len {
            //alternate between corners
            let pos_x = match count % 2 {
                0 => 0.0f64,
                _ => (stage_size.width - img_size.width) as f64
            };

            let pos_y = (stage_size.height - img_size.height) as f64;
            let position = Point { x: pos_x, y: pos_y };

            let mut speed = Point::new_random();

            speed.x *= 10.0;
            speed.y = (speed.y * 10.0) - 5.0;

            entities.add_entity((&mut positions, &mut speeds, &mut gravities), (Position(position), Speed(speed), Gravity(START_GRAVITY)));
        }

        instance_positions.0.resize(len * 2, 0.0);
    }
}

#[system(Update)]
pub fn run (mut fps_counter: Unique<&mut FpsCounter>) {
    fps_counter.begin();
}

#[system(Render)]
pub fn run (
    positions: &Position, 
    speeds:&Speed, 
    gravities:&Gravity, 
    stage_area:Unique<&StageArea>, 
    img_area:Unique<&ImageArea>,
    instance_positions:Unique<&InstancePositions>
) {
    let len = positions.len();
}


#[system(End)]
pub fn run (mut fps_counter: Unique<&mut FpsCounter>, positions:&Position) {
    fps_counter.end();
    let fps = fps_counter.current.ceil() as u32;
    let len = positions.len();
    //hud.update(len, fps);
}