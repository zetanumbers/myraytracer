const TAU: f32 = 6.2831853;

struct VertexOutput {
    @location(0) pixel_pos: vec2<f32>,
    @builtin(position) pos: vec4<f32>,
}

struct Locals {
    shape: vec2<i32>,
    sample_count: u32,
    depth: u32,
    rng_shuffle: vec4<u32>,
    weight_framebuffer: f32,
    _padding1: i32,
    _padding2: i32,
    _padding3: i32,
}

@group(0) @binding(0)
var<uniform> r_locals: Locals;

@vertex
fn vs_main(@location(0) vertex: vec2<f32>) -> VertexOutput {
    let pixel_pos = (0.5 * vertex + vec2<f32>(0.5)) * vec2<f32>(r_locals.shape);
    
    return VertexOutput(pixel_pos, vec4<f32>(vertex, 0.0, 1.0));
}

@group(1) @binding(0)
var r_framebuffer: texture_2d<f32>;

fn framebuffer_load(pixel_pos: vec2<f32>) -> vec4<f32> {
    let pixel_pos_clamped = clamp(vec2<i32>(pixel_pos), vec2<i32>(0), r_locals.shape - vec2<i32>(1));
    return textureLoad(r_framebuffer, pixel_pos_clamped, 0);
}


@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return framebuffer_load(in.pixel_pos);
}
