use crate::geometry::*;
use std::rc::Rc;
use std::cell::RefCell;
use lazy_static::lazy_static;
use web_sys::{HtmlImageElement};
use nalgebra::{Matrix4, Vector3};

use awsm_web::webgl::{
    ClearBufferMask,
    WebGl1Renderer,
    AttributeOptions,
    BufferData,
    BufferTarget,
    BufferUsage,
    DataType,
    TextureTarget,
    PixelFormat,
    SimpleTextureOptions,
    WebGlTextureSource,
    Id,
    BeginMode,
    GlToggle,
    BlendFactor,
};

pub struct SceneRenderer {
    renderer: WebGl1Renderer,
    ids: SceneIds,
}

struct SceneIds {
    program_id: Id,
    texture_id: Id,
    instance_id: Id,
}
impl SceneRenderer {
    pub fn new (mut renderer:WebGl1Renderer, vertex:&str, fragment:&str, img:&HtmlImageElement) -> Result<Self, awsm_web::errors::Error> {
        let ids = {
            //This demo is specifically using webgl1, which needs to register the extension
            //Everything else is the same API as webgl2 :)
            renderer.register_extension_instanced_arrays()?;

            //compile the shaders and get a program id
            let program_id = renderer.compile_program(vertex, fragment)?;

            //create quad data and get a buffer id
            let geom_id = renderer.create_buffer()?;

            renderer.upload_buffer_to_attribute(
                geom_id,
                BufferData::new(
                    &QUAD_GEOM_UNIT,
                    BufferTarget::ArrayBuffer,
                    BufferUsage::StaticDraw,
                    ),
                    "a_vertex",
                    &AttributeOptions::new(2, DataType::Float),
                    )?;

            //create texture data and get a texture id
            let texture_id = renderer.create_texture()?;
            renderer.assign_simple_texture(
                texture_id,
                TextureTarget::Texture2d,
                &SimpleTextureOptions {
                    pixel_format: PixelFormat::Rgba,
                    ..SimpleTextureOptions::default()
                },
                &WebGlTextureSource::ImageElement(&img),
                )?;

            //create an instance buffer and get the id
            let instance_id = renderer.create_buffer()?;

            SceneIds {program_id, texture_id, instance_id }
        };

        Ok(Self { renderer, ids} )
    }

    pub fn render(&mut self, len:usize, img_area:Area, stage_area:Area, instance_positions:&[f32]) -> Result<(), awsm_web::errors::Error> {
        if len == 0 {
            return Ok(());
        }

        let renderer = &mut self.renderer;
        let SceneIds {program_id, texture_id, instance_id, ..} = self.ids;


        //Clear the screen buffers
        renderer.clear(&[
                ClearBufferMask::ColorBufferBit,
                ClearBufferMask::DepthBufferBit,
        ]);

        //set blend mode. this will be a noop internally if already set
        renderer.toggle(GlToggle::Blend, true);
        renderer.toggle(GlToggle::DepthTest, false);
        renderer.set_blend_func(BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha);

        //will already be activated but internally that's a noop if true
        renderer.activate_program(program_id)?;

        //enable texture
        renderer.activate_texture_for_sampler(texture_id, "u_sampler")?;

        //Build our matrices (must cast to f32)
        let scaling_mat = Matrix4::new_nonuniform_scaling(&Vector3::new(img_area.width as f32, img_area.height as f32, 0.0));
        let camera_mat = Matrix4::new_orthographic( 0.0, stage_area.width as f32, 0.0, stage_area.height as f32, 0.0, 1.0);

        //Upload them to the GPU
        renderer.upload_uniform_mat_4("u_size", &scaling_mat.as_slice())?;
        renderer.upload_uniform_mat_4("u_camera", &camera_mat.as_slice())?;


        //need the location for the attrib_divisor below
        let loc = renderer.get_attribute_location_value("a_position")?;
        //upload instance positions
        renderer.upload_buffer( instance_id, BufferData::new(
                &instance_positions,
                BufferTarget::ArrayBuffer,
                BufferUsage::StaticDraw,
        ))?;

        renderer.activate_attribute_loc(loc, &AttributeOptions::new(2, DataType::Float));

        renderer.vertex_attrib_divisor(loc, 1)?;
        renderer.draw_arrays_instanced(BeginMode::TriangleStrip, 0, 4, len as u32)?;

        Ok(())
    }

}
