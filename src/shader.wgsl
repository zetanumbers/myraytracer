struct VertexOutput {
    [[location(0)]] ray_dir_denorm: vec3<f32>;
    [[location(1)]] pixel_pos: vec2<f32>;
    [[builtin(position)]] pos: vec4<f32>;
};

struct Locals {
    shape: vec2<i32>;
    sample_count: u32;
    _padding: u32;
};

[[group(0), binding(0)]]
var<uniform> r_locals: Locals;

[[stage(vertex)]]
fn vs_main([[location(0)]] vertex: vec2<f32>) -> VertexOutput {
    let focal_length = 1.0;
    let viewport_quad_shape = vec2<f32>(1.0, f32(r_locals.shape.x) / f32(r_locals.shape.y));
    
    let ray_dir_denorm = vec3<f32>(viewport_quad_shape * vertex, focal_length);
    
    let pixel_pos = (0.5 * vertex + vec2<f32>(0.5)) * vec2<f32>(r_locals.shape);
    
    return VertexOutput(ray_dir_denorm, pixel_pos, vec4<f32>(vertex, 0.0, 1.0));
}

[[group(0), binding(1)]]
var r_rands: texture_2d<u32>;

fn rotl_u32(x: u32, k: u32) -> u32 {
    return (x << k) | (x >> (32u - k));
}

fn xoshiro128plus_next_u32(s: ptr<function, vec4<u32>>) -> u32 {
    let result = (*s)[0] + (*s)[3];
    
    let t = (*s)[1] << 9u;
    
    (*s)[2] = (*s)[2] ^ (*s)[0];
    (*s)[3] = (*s)[3] ^ (*s)[1];
    (*s)[1] = (*s)[1] ^ (*s)[2];
    (*s)[0] = (*s)[0] ^ (*s)[3];
    
    (*s)[2] = (*s)[2] ^ t;
    
    (*s)[3] = rotl_u32((*s)[3], 11u);
    
    return result;
}

fn xoshiro128plus_next_f32(s: ptr<function, vec4<u32>>) -> f32 {
    return f32(xoshiro128plus_next_u32(s) >> 8u) / 16777216.0;
}

fn xoshiro128plus_next_vec2_f32(s: ptr<function, vec4<u32>>) -> vec2<f32> {
    let x = xoshiro128plus_next_f32(s);
    let y = xoshiro128plus_next_f32(s);
    return vec2<f32>(x, y);
}

fn xoshiro128plus_next_vec3_f32(s: ptr<function, vec4<u32>>) -> vec3<f32> {
    let x = xoshiro128plus_next_f32(s);
    let y = xoshiro128plus_next_f32(s);
    let z = xoshiro128plus_next_f32(s);
    return vec3<f32>(x, y, z);
}

struct Ray {
    orig: vec3<f32>;
    dir: vec3<f32>;
};

fn color_world(ray_norm: Ray) -> vec3<f32> {
    let t = 0.5 * ray_norm.dir.y + 0.5;
    return mix(vec3<f32>(1.0), vec3<f32>(0.5, 0.7, 1.0), t);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let focal_length = 1.0;
    let shape = vec2<i32>(r_locals.shape);
    let pixel_pos = clamp(vec2<i32>(in.pixel_pos), vec2<i32>(0), shape - vec2<i32>(1));

    var rand: vec4<u32> = textureLoad(r_rands, pixel_pos, 0);
    
    var color: vec3<f32> = vec3<f32>(0.0);
    
    let sample_shape = vec2<f32>(2.0 / f32(shape.y));
    
    for (var i: u32 = 0u; i < r_locals.sample_count; i = i + 1u) {
        let rand_offset = xoshiro128plus_next_vec2_f32(&rand);
        let ray_dir_denorm = in.ray_dir_denorm + vec3<f32>(rand_offset * sample_shape, focal_length);
        color = color + color_world(Ray(vec3<f32>(0.0), normalize(ray_dir_denorm)));
    }
    color = color / f32(r_locals.sample_count);
    
    return vec4<f32>(color, 1.0);
}
