use crate::rand::gen_range;
use macroquad::{
    input::show_mouse,
    prelude::*,
    ui::{root_ui, widgets::Button},
};
use shipyard::{
    iter::IntoIter, sparse_set::SparseSet, AddComponent, AllStorages, AllStoragesViewMut,
    Component, EntitiesViewMut, IntoWorkload, IntoWorkloadTrySystem, Unique, UniqueView,
    UniqueViewMut, View, ViewMut, Workload, World,
};

const WIDTH: f32 = 640.0;
const HEIGHT: f32 = 360.0;
const BASE_INIT_SIZE: f32 = 5.;
const MAX_SIZE: f32 = 25.;
const BASE_GROWTH_RATE: f32 = 0.15;
const BASE_SPEED: f32 = 1.5;
const ACCELERATION_RATE: f32 = 0.002;
const BASE_SQUARE_SPAWN_RATE: u32 = 25;
const BASE_SQUAGUM_SPAWN_RATE: u32 = 150;

#[derive(Component)]
struct Square {
    x: f32,
    y: f32,
    size: f32,
}

impl Square {
    fn pos(&self) -> Vec2 {
        vec2(self.x + self.size / 2.0, self.y + self.size / 2.0)
    }

    fn collide(&self, other: &Square) -> bool {
        self.x + self.size >= other.x
            && self.x <= other.x + other.size
            && self.y + self.size >= other.y
            && self.y <= other.y + other.size
    }
}

#[derive(Unique)]
struct Player {
    is_invincible: bool,
    i_counter: u32,
    squagum: bool,
    squagum_counter: u32,
    square: Square,
}

#[derive(Component)]
struct Squagum(Vec2);
#[derive(Component)]
struct Acceleration(f32);
#[derive(Component)]
struct ToDelete;

#[derive(Debug, Component)]
enum FloorResult {
    Loose,
    Win,
}

impl std::error::Error for FloorResult {}

impl std::fmt::Display for FloorResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Unique)]
struct FloorCounter(u32);

#[derive(Unique)]
struct MaxFloor(u32);

#[derive(Unique)]
struct SpawnedOnFloor(u32);

#[derive(Unique, Debug)]
struct PowerUps {
    player_start_size: u32,
    player_boost_duration: u32,
    player_boost_spawn_rate: u32,
    player_size_on_eat: u32,
    player_defense: u32,
    square_start_size: u32,
    square_growth_rate: u32,
    square_speed: u32,
    square_number: u32,
    square_spawn_rate: u32,
}

impl PowerUps {
    fn new() -> PowerUps {
        PowerUps {
            player_start_size: 0,
            player_boost_duration: 0,
            player_boost_spawn_rate: 0,
            player_size_on_eat: 0,
            player_defense: 0,
            square_start_size: 0,
            square_growth_rate: 0,
            square_speed: 0,
            square_number: 0,
            square_spawn_rate: 0,
        }
    }

    fn player_start_size(&self) -> f32 {
        self.player_start_size as f32 * 3.0
    }

    fn player_boost_duration(&self) -> u32 {
        self.player_boost_duration * 10
    }

    fn player_boost_spawn_rate(&self) -> u32 {
        self.player_boost_spawn_rate * 10
    }

    fn player_size_on_eat(&self) -> f32 {
        self.player_size_on_eat as f32 * 0.5
    }

    fn player_defense(&self) -> f32 {
        self.player_defense as f32 * 0.4
    }

    fn square_start_size(&self) -> f32 {
        self.square_start_size as f32 * 0.5
    }

    fn square_growth_rate(&self) -> f32 {
        self.square_growth_rate as f32 * 0.05
    }

    fn square_speed(&self) -> f32 {
        self.square_speed as f32 * 0.1
    }

    fn square_number(&self) -> u32 {
        self.square_number * 3
    }

    fn square_spawn_rate(&self) -> u32 {
        self.square_spawn_rate * 4
    }

    fn iter_player_power_ups(&mut self) -> impl IntoIterator<Item = (&'static str, u32, &mut u32)> {
        [
            ("Start size", 3, &mut self.player_start_size),
            ("Boost duration", 10, &mut self.player_boost_duration),
            ("Boost spawn rate", 10, &mut self.player_boost_spawn_rate),
            ("Size on eat", 10, &mut self.player_size_on_eat),
            ("Defense", 5, &mut self.player_defense),
        ]
    }
}

/// generates a new random square.
fn new_square(init_size_boost: f32) -> (Square, Acceleration) {
    (
        Square {
            x: gen_range(0.0, WIDTH - BASE_INIT_SIZE),
            y: gen_range(0.0, HEIGHT - BASE_INIT_SIZE),
            size: BASE_INIT_SIZE + init_size_boost,
        },
        Acceleration(0.),
    )
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Square Eater".to_owned(),
        window_width: WIDTH as i32,
        window_height: HEIGHT as i32,
        ..Default::default()
    }
}

fn clear_floor(all_storages: &mut AllStorages) {
    all_storages.delete_any::<SparseSet<Square>>();
    all_storages.delete_any::<SparseSet<Squagum>>();
}

fn init_floor(
    mut entities: EntitiesViewMut,
    floor_counter: UniqueView<FloorCounter>,
    mut player: UniqueViewMut<Player>,
    power_ups: UniqueView<PowerUps>,
    mut spawned_on_floor: UniqueViewMut<SpawnedOnFloor>,
    mut accelerations: ViewMut<Acceleration>,
    mut squares: ViewMut<Square>,
) {
    *player = Player {
        is_invincible: false,
        i_counter: 0,
        squagum: false,
        squagum_counter: 0,
        square: Square {
            x: WIDTH / 2.0 - BASE_INIT_SIZE * 1.5,
            y: HEIGHT / 2.0 - BASE_INIT_SIZE * 1.5,
            size: BASE_INIT_SIZE * 3.0 + power_ups.player_start_size(),
        },
    };

    entities.bulk_add_entity(
        (&mut squares, &mut accelerations),
        (0..floor_counter.0).map(|_| new_square(power_ups.square_start_size())),
    );

    spawned_on_floor.0 = floor_counter.0;
}

fn place_buttons(world: &World) {
    let mut root_ui = root_ui();
    let mut should_transition = false;

    world.run(|mut power_ups: UniqueViewMut<PowerUps>| {
        for (i, (text, max_power_level, power_up)) in
            power_ups.iter_player_power_ups().into_iter().enumerate()
        {
            let height_offset = i as f32 * 25.0;

            let text_dimensions = measure_text(text, None, 20, 1.0);
            draw_text(text, WIDTH / 8.0, HEIGHT / 4.0 + height_offset, 20.0, BLACK);

            if *power_up < max_power_level
                && Button::new("+")
                    .position(vec2(
                        WIDTH / 8.0 + text_dimensions.width + 5.0,
                        HEIGHT / 4.0 - text_dimensions.height + height_offset,
                    ))
                    .size(vec2(15.0, 15.0))
                    .ui(&mut *root_ui)
            {
                *power_up += 1;
                should_transition = true;
                return;
            }
        }
    });

    if should_transition {
        world.run_with_data(transition_screen, Screen::Floor);
    }
}

fn floor_loop() -> Workload {
    (
        counters,
        move_player,
        move_square,
        grow_square,
        spawn,
        collision,
        clean_up,
        check_end_floor.into_workload_try_system().unwrap(),
        render,
    )
        .into_workload()
}

#[derive(Unique, Clone, Copy)]
enum Screen {
    Start,
    Floor,
    Shop,
}

fn transition_screen(new_screen: Screen, mut all_storages: AllStoragesViewMut) {
    match new_screen {
        Screen::Start => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                show_mouse(true);
                set_cursor_grab(false);
            }
        }
        Screen::Floor => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                show_mouse(false);
                set_cursor_grab(true);
            }
            all_storages.run(init_floor);
        }
        Screen::Shop => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                show_mouse(true);
                set_cursor_grab(false);
            }
            clear_floor(&mut all_storages);
        }
    }

    all_storages.run(|mut screen: UniqueViewMut<Screen>| {
        *screen = new_screen;
    });
}

// Entry point of the program
#[macroquad::main(window_conf)]
async fn main() {
    let world = World::new();

    world.add_unique(FloorCounter(1));
    world.add_unique(SpawnedOnFloor(0));
    world.add_unique(Player {
        is_invincible: false,
        i_counter: 0,
        squagum: false,
        squagum_counter: 0,
        square: Square {
            x: 0.0,
            y: 0.0,
            size: 0.0,
        },
    });
    world.add_unique(PowerUps::new());
    world.add_unique(MaxFloor(1));
    world.add_unique(Screen::Start);
    world.run(init_floor);

    // seed the random number generator with a random value
    rand::srand(macroquad::miniquad::date::now() as u64);

    world.add_workload(floor_loop);

    loop {
        let screen = *world.borrow::<UniqueView<Screen>>().unwrap();

        match screen {
            Screen::Start => {
                if is_mouse_button_pressed(MouseButton::Left) {
                    world.run_with_data(transition_screen, Screen::Floor);
                    continue;
                }

                clear_background(BLACK);

                let text_dimensions = measure_text("Click to start", None, 40, 1.);
                draw_text(
                    "Click to start",
                    WIDTH / 2. - text_dimensions.width / 2.,
                    HEIGHT / 2. - text_dimensions.height / 2.,
                    40.,
                    WHITE,
                );
            }
            Screen::Floor => {
                clear_background(WHITE);

                if let Err(Some(err)) = world
                    .run_workload(floor_loop)
                    .map_err(shipyard::error::RunWorkload::custom_error)
                {
                    match err.downcast_ref::<FloorResult>().unwrap() {
                        FloorResult::Loose => {
                            debug!("Loose");
                            world.run(|mut floor_counter: UniqueViewMut<FloorCounter>| {
                                floor_counter.0 -= 1;
                                floor_counter.0 = floor_counter.0.max(1);
                            });
                        }
                        FloorResult::Win => {
                            debug!("Win");
                            world.run(|mut floor_counter: UniqueViewMut<FloorCounter>, mut max_floor: UniqueViewMut<MaxFloor>, mut power_ups: UniqueViewMut<PowerUps>| {
                                floor_counter.0 += 1;
                                max_floor.0 = max_floor.0.max(floor_counter.0);

                                if floor_counter.0 == max_floor.0 {
                                    let power = if power_ups.square_spawn_rate < 5 {
                                        gen_range(0, 5)
                                    } else {
                                        gen_range(0, 4)
                                    };
                                    match power {
                                        0 => {
                                            power_ups.square_start_size += 1;
                                        }
                                        1 => {
                                            power_ups.square_growth_rate += 1;
                                        }
                                        2 => {
                                            power_ups.square_speed += 1;
                                        }
                                        3 => {
                                            power_ups.square_number += 1;
                                        }
                                        4 => {
                                            power_ups.square_spawn_rate += 1;
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                            });
                        }
                    }

                    world.run_with_data(transition_screen, Screen::Shop);
                }
            }
            Screen::Shop => {
                clear_background(WHITE);

                place_buttons(&world);

                let mut root_ui = root_ui();
                let text_dimensions = measure_text("Skip", None, 30, 1.0);

                if Button::new("Skip")
                    .size(vec2(
                        text_dimensions.width + 10.0,
                        text_dimensions.height + 10.0,
                    ))
                    .position(vec2(
                        WIDTH / 2.0 - (text_dimensions.width + 10.0) / 2.0,
                        HEIGHT * 3.0 / 4.0,
                    ))
                    .ui(&mut root_ui)
                {
                    world.run_with_data(transition_screen, Screen::Floor);
                    world.run(init_floor);
                }
            }
        }

        next_frame().await
    }
}

fn counters(mut player: UniqueViewMut<Player>, power_ups: UniqueView<PowerUps>) {
    if player.is_invincible {
        player.i_counter += 1;

        if player.i_counter >= 10 {
            player.is_invincible = false;
            player.i_counter = 0;
        }
    }

    if player.squagum {
        player.squagum_counter += 1;

        if player.squagum_counter >= 120 + power_ups.player_boost_duration() {
            player.squagum = false;
            player.squagum_counter = 0;
        }
    }
}

fn move_player(mut player: UniqueViewMut<Player>) {
    let (x, y) = mouse_position();
    player.square.x = x.clamp(0.0, WIDTH - player.square.size);
    player.square.y = y.clamp(0.0, HEIGHT - player.square.size);
}

fn move_square(
    player: UniqueView<Player>,
    mut accelerations: ViewMut<Acceleration>,
    power_ups: UniqueView<PowerUps>,
    mut squares: ViewMut<Square>,
) {
    for acceleration in (&mut accelerations).iter() {
        acceleration.0 += ACCELERATION_RATE;
    }

    let mut dirs = vec![Vec2::ZERO; squares.len()];

    for ((square, acceleration), dir) in (&squares, &mut accelerations).iter().zip(&mut dirs) {
        if square.size > player.square.size {
            let player_dir = player.square.pos() - square.pos();

            *dir = player_dir.normalize();

            if player.squagum {
                *dir = -*dir;
            }

            let mut neighbourg_dir = Vec2::ZERO;

            for neighbourg in squares.iter() {
                if square.pos().distance_squared(neighbourg.pos()) < square.size * square.size / 1.5
                {
                    neighbourg_dir += Vec2::new(square.x - neighbourg.x, square.y - neighbourg.y);
                }
            }

            if square.size == MAX_SIZE {
                *dir *= BASE_SPEED + power_ups.square_speed() + acceleration.0;
            } else {
                *dir *= BASE_SPEED + power_ups.square_speed();
            }

            *dir += neighbourg_dir * 0.05;
        }
    }

    for (square, dir) in (&mut squares).iter().zip(dirs) {
        if dir != Vec2::ZERO {
            square.x = (square.x + dir.x).clamp(0.0, WIDTH - square.size);
            square.y = (square.y + dir.y).clamp(0.0, HEIGHT - square.size);
        }
    }
}

fn grow_square(power_ups: UniqueView<PowerUps>, mut squares: ViewMut<Square>) {
    for rect in (&mut squares).iter() {
        let delta_size = (rect.size + BASE_GROWTH_RATE + power_ups.square_growth_rate())
            .min(MAX_SIZE)
            - rect.size;
        rect.size = rect.size + delta_size;
        rect.x = (rect.x - delta_size / 2.0).max(0.0);
        rect.y = (rect.y - delta_size / 2.0).max(0.0);
    }
}

fn spawn(
    mut entities: EntitiesViewMut,
    floor_counter: UniqueView<FloorCounter>,
    power_ups: UniqueView<PowerUps>,
    mut spawned_on_floor: UniqueViewMut<SpawnedOnFloor>,
    mut accelerations: ViewMut<Acceleration>,
    mut squagums: ViewMut<Squagum>,
    mut squares: ViewMut<Square>,
) {
    let should_spawn = spawned_on_floor.0 < (floor_counter.0 + 1) * 2 + power_ups.square_number();
    if should_spawn
        && rand::gen_range(0, BASE_SQUARE_SPAWN_RATE - power_ups.square_spawn_rate()) == 0
    {
        entities.add_entity(
            (&mut squares, &mut accelerations),
            new_square(power_ups.square_start_size()),
        );
        spawned_on_floor.0 += 1;
    }

    if rand::gen_range(
        0,
        BASE_SQUAGUM_SPAWN_RATE - power_ups.player_boost_spawn_rate(),
    ) == 0
    {
        entities.add_entity(
            &mut squagums,
            Squagum(Vec2::new(
                rand::gen_range(0.0, WIDTH - BASE_INIT_SIZE * 2.0),
                rand::gen_range(0.0, HEIGHT - BASE_INIT_SIZE * 2.0),
            )),
        );
    }
}

fn collision(
    mut player: UniqueViewMut<Player>,
    power_ups: UniqueView<PowerUps>,
    squares: View<Square>,
    squagums: View<Squagum>,
    mut to_delete: ViewMut<ToDelete>,
) {
    for (id, squagum) in squagums.iter().with_id() {
        if player.square.collide(&Square {
            x: squagum.0.x,
            y: squagum.0.y,
            size: BASE_INIT_SIZE,
        }) {
            player.squagum = true;
            player.squagum_counter = 0;
            to_delete.add_component_unchecked(id, ToDelete);
        }
    }

    for (id, square) in squares.iter().with_id() {
        if square.size == MAX_SIZE && square.collide(&player.square) {
            if player.squagum {
                player.square.size =
                    (player.square.size + BASE_INIT_SIZE / 4. + power_ups.player_size_on_eat())
                        .min(MAX_SIZE - 0.01);
                to_delete.add_component_unchecked(id, ToDelete);
            }

            if !player.is_invincible {
                player.is_invincible = true;
                player.square.size -= BASE_INIT_SIZE / 2.;
                player.square.size += power_ups.player_defense();
            }
        } else if player.square.size >= square.size && player.square.collide(&square) {
            player.square.size =
                (player.square.size + BASE_INIT_SIZE / 2. + power_ups.player_size_on_eat())
                    .min(MAX_SIZE - 0.01);
            to_delete.add_component_unchecked(id, ToDelete)
        }
    }
}

fn clean_up(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<ToDelete>>();
}

fn check_end_floor(
    floor_counter: UniqueView<FloorCounter>,
    player: UniqueView<Player>,
    power_ups: UniqueView<PowerUps>,
    spawned_on_floor: UniqueView<SpawnedOnFloor>,
    squares: ViewMut<Square>,
) -> Result<(), FloorResult> {
    if player.square.size < BASE_INIT_SIZE {
        Err(FloorResult::Loose)
    } else if spawned_on_floor.0 == (floor_counter.0 + 1) * 2 + power_ups.square_number()
        && squares.is_empty()
    {
        Err(FloorResult::Win)
    } else {
        Ok(())
    }
}

fn render(
    player: UniqueView<Player>,
    squares: View<Square>,
    squagums: View<Squagum>,
    floor_counter: UniqueView<FloorCounter>,
) {
    for square in squares.iter() {
        draw_rectangle(
            square.x,
            square.y,
            square.size,
            square.size,
            if square.size == MAX_SIZE {
                RED
            } else if square.size > player.square.size {
                GRAY
            } else {
                GREEN
            },
        );
    }

    for squagum in squagums.iter() {
        draw_rectangle(
            squagum.0.x,
            squagum.0.y,
            BASE_INIT_SIZE * 2.0,
            BASE_INIT_SIZE * 2.0,
            YELLOW,
        );
    }

    draw_rectangle(
        player.square.x,
        player.square.y,
        player.square.size,
        player.square.size,
        if player.squagum { YELLOW } else { BLUE },
    );

    let floor_number = floor_counter.0.to_string();
    let text_dimensions = measure_text(&floor_number, None, 35, 1.);
    draw_text(
        &floor_number,
        WIDTH - text_dimensions.width,
        18.,
        35.,
        BLACK,
    );
}
