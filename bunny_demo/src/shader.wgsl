// this needs to be 16 bytes aligned for webgl https://github.com/gfx-rs/wgpu/issues/2832
@group(0) @binding(0)
var<uniform> screen_size: vec4<f32>;

struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};
struct Instance {
    @location(2) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u64,
    vertex: Vertex,
    instance: Instance,
) -> VertexOutput {
    var out: VertexOutput;
    out.tex_coords = vertex.tex_coords;

    var pos = vertex.position;
    pos += instance.position;

    pos.x *= 2.0;
    pos.y *= 2.0;
    pos.x /= screen_size.x;
    pos.y /= screen_size.y;
    pos.x -= 1.0;
    pos.y -= 1.0;

    out.clip_position = vec4<f32>(pos.x, pos.y, 0.0, 1.0);

    return out;
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    //return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    return textureSample(t_diffuse, s_diffuse, in.tex_coords);
}
