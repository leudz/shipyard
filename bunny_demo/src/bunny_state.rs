use crate::graphics::Graphics;
use glam::Vec2;
use nanorand::Rng;
use shipyard::{
    Component, EntitiesViewMut, IntoIter, Unique, UniqueView, UniqueViewMut, View, ViewMut,
};
use wgpu::util::DeviceExt;

const IMG_SIZE: [u64; 2] = [25, 32];

#[derive(Unique)]
pub(crate) struct BunnyState {
    pub(crate) bunnies_per_click: u64,
}

#[repr(transparent)]
#[derive(Component, Copy, Clone, Debug)]
#[track(Insertion)]
pub(crate) struct BunnyPosition(Vec2);

impl BunnyPosition {
    pub(crate) fn new(pos: Vec2) -> BunnyPosition {
        BunnyPosition(pos)
    }
}

unsafe impl bytemuck::Pod for BunnyPosition {}
unsafe impl bytemuck::Zeroable for BunnyPosition {}

impl BunnyPosition {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![2 => Float32x2];

    pub(crate) const fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vec2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[derive(Component)]
pub(crate) struct Vel(Vec2);

pub(crate) fn add_bunnies(
    graphics: UniqueView<Graphics>,
    bunny_state: UniqueView<BunnyState>,
    mut entities: EntitiesViewMut,
    mut position: ViewMut<BunnyPosition>,
    mut velocity: ViewMut<Vel>,
) {
    entities.bulk_add_entity(
        (&mut position, &mut velocity),
        (0..bunny_state.bunnies_per_click).map(|count| {
            //alternate between corners
            let pos_x = match count % 2 {
                0 => 0.0,
                _ => (graphics.size.width - IMG_SIZE[0]) as f32,
            };

            let pos_y = (graphics.size.height - IMG_SIZE[1]) as f32;
            let position = Vec2 { x: pos_x, y: pos_y };

            let mut speed = Vec2 {
                x: nanorand::tls_rng().generate(),
                y: nanorand::tls_rng().generate(),
            };

            speed.x *= 10.0;
            speed.y = (speed.y * 10.0) - 5.0;

            (BunnyPosition::new(position), Vel(speed))
        }),
    );
}

pub(crate) fn simulate_bunnies(
    graphics: UniqueView<Graphics>,
    mut position: ViewMut<BunnyPosition>,
    mut velocity: ViewMut<Vel>,
) {
    (&mut position, &mut velocity)
        .iter()
        .for_each(|(BunnyPosition(pos), Vel(speed))| {
            let gravity = -0.75;

            //movement is made to match https://github.com/pixijs/bunny-mark/blob/master/src/Bunny.js
            *pos += *speed;
            speed.y += gravity;

            let bounds_right = (graphics.size.width - IMG_SIZE[0]) as f32;
            if pos.x > bounds_right {
                speed.x *= -1.0;
                pos.x = bounds_right;
            } else if pos.x < 0.0 {
                speed.x *= -1.0;
                pos.x = 0.0;
            }

            let bounds_top = (graphics.size.height - IMG_SIZE[1]) as f32;

            if pos.y < 0.0 {
                speed.y *= -0.85;
                pos.y = 0.0;
                let rand_bool: bool = nanorand::tls_rng().generate();
                if rand_bool {
                    let rand_float: f32 = nanorand::tls_rng().generate();
                    speed.y -= rand_float * 6.0;
                }
            } else if pos.y > bounds_top {
                speed.y = 0.0;
                pos.y = bounds_top;
            }
        });
}

pub(crate) fn update_bunnies_gpu(
    mut graphics: UniqueViewMut<Graphics>,
    position: View<BunnyPosition>,
) {
    if position.inserted().iter().next().is_some() {
        let instance_buffer =
            graphics
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(position.as_slice()),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        graphics.instance_buffer = instance_buffer;
        graphics.vertex_count = position.len() as u64;
    } else {
        graphics.queue.write_buffer(
            &graphics.instance_buffer,
            0,
            bytemuck::cast_slice(position.as_slice()),
        );
    }
}
