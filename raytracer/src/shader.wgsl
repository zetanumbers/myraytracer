let TAU: f32 = 6.2831853;

struct VertexOutput {
    [[location(0)]] pixel_pos: vec2<f32>;
    [[builtin(position)]] pos: vec4<f32>;
};

struct Locals {
    shape: vec2<i32>;
    sample_count: u32;
    depth: u32;
};

[[group(0), binding(0)]]
var<uniform> r_locals: Locals;

[[stage(vertex)]]
fn vs_main([[location(0)]] vertex: vec2<f32>) -> VertexOutput {
    let viewport_quad_shape = vec2<f32>(f32(r_locals.shape.x) / f32(r_locals.shape.y), 1.0);
    
    let pixel_pos = (0.5 * vertex + vec2<f32>(0.5)) * vec2<f32>(r_locals.shape);
    
    return VertexOutput(pixel_pos, vec4<f32>(vertex, 0.0, 1.0));
}

// Random

[[group(0), binding(1)]]
var r_rands: texture_2d<u32>;

fn rotl_u32(x: u32, k: u32) -> u32 {
    return (x << k) | (x >> (32u - k));
}

struct Xoshiro128Plus {
    state: vec4<u32>;
};

fn xoshiro128plus_load(pixel_pos: vec2<f32>) -> Xoshiro128Plus {
    let pixel_pos = clamp(vec2<i32>(pixel_pos), vec2<i32>(0), r_locals.shape - vec2<i32>(1));
    return Xoshiro128Plus(textureLoad(r_rands, pixel_pos, 0));
}

fn xoshiro128plus_random_u32(rng: ptr<function, Xoshiro128Plus>) -> u32 {
    let result = (*rng).state[0] + (*rng).state[3];
    
    let t = (*rng).state[1] << 9u;
    
    (*rng).state[2] = (*rng).state[2] ^ (*rng).state[0];
    (*rng).state[3] = (*rng).state[3] ^ (*rng).state[1];
    (*rng).state[1] = (*rng).state[1] ^ (*rng).state[2];
    (*rng).state[0] = (*rng).state[0] ^ (*rng).state[3];
    
    (*rng).state[2] = (*rng).state[2] ^ t;
    
    (*rng).state[3] = rotl_u32((*rng).state[3], 11u);
    
    return result;
}

fn xoshiro128plus_random_f32(rng: ptr<function, Xoshiro128Plus>) -> f32 {
    let i = xoshiro128plus_random_u32(rng);
    return f32(i) / 4294967296.0;
}

fn xoshiro128plus_random_vec2_f32(rng: ptr<function, Xoshiro128Plus>) -> vec2<f32> {
    let x = xoshiro128plus_random_f32(rng);
    let y = xoshiro128plus_random_f32(rng);
    return vec2<f32>(x, y);
}

fn xoshiro128plus_random_vec3_f32(rng: ptr<function, Xoshiro128Plus>) -> vec3<f32> {
    let x = xoshiro128plus_random_f32(rng);
    let y = xoshiro128plus_random_f32(rng);
    let z = xoshiro128plus_random_f32(rng);
    return vec3<f32>(x, y, z);
}

fn xoshiro128plus_random_unit_ball_vec3_f32(rng: ptr<function, Xoshiro128Plus>) -> vec3<f32> {
    var v: vec3<f32> = 2.0 * xoshiro128plus_random_vec3_f32(rng) - vec3<f32>(1.0);
    for (; dot(v, v) > 1.0; ) {
        v = 2.0 * xoshiro128plus_random_vec3_f32(rng) - vec3<f32>(1.0);
    }
    return v;
}

fn xoshiro128plus_random_unit_sphere_vec3_f32(rng: ptr<function, Xoshiro128Plus>) -> vec3<f32> {
    return normalize(xoshiro128plus_random_unit_ball_vec3_f32(rng));
}

// Render

struct Ray {
    orig: vec3<f32>;
    dir: vec3<f32>;
};

fn ray_normalized_at(r: ptr<function, Ray>, t: f32) -> vec3<f32> {
    return (*r).orig + t * (*r).dir;
}

// Materials

struct LambertianRange {
    // vec3<f32>
    albedo_base_idx: i32;
    length: i32;
    _padding2: i32;
    _padding3: i32;
};

struct MetalRange {
    // vec3<f32>
    albedo_base_idx: i32;
    // f32
    fuzz_base_idx: i32;
    length: i32;
    _padding3: i32;
};

let LAMBERTIAN_MATERIAL_TYPE: i32 = 1;
let METAL_MATERIAL_TYPE: i32 = 2;

struct DynMaterial {
    ty: i32;
    idx: i32;
};

struct Hit {
    at: vec3<f32>;
    t: f32;
    normal: vec3<f32>;
    front_face: bool;
    material: DynMaterial;
};

fn hit_nil() -> Hit {
    return Hit(vec3<f32>(0.0), 0.0, vec3<f32>(0.0), false, DynMaterial(0, 0));
}

struct HitArgs {
    ray_norm: Ray;
    t_min: f32;
    t_sup: f32;
};

struct ScatterOutput {
    attenuation: vec3<f32>;
    // could be denormal
    ray: Ray;
};

struct ScatterArgs {
    ray: Ray;
    hit: Hit;
};

// Primitives

struct SphereRange {
    // vec3<f32>
    center_base_idx: i32;
    // vec3<f32>
    radius_base_idx: i32;
    material_ty_base_idx: i32;
    material_idx_base_idx: i32;
    length: i32;
    _padding1: i32;
    _padding2: i32;
    _padding3: i32;
};

struct World {
    spheres: SphereRange;
    lambertians: LambertianRange;
    metals: MetalRange;
};

[[group(1), binding(0)]]
var<uniform> r_world: World;

// Data arrays

[[group(1), binding(1)]]
var r_vec4_f32_data: texture_1d<f32>;

[[group(1), binding(2)]]
var r_f32_data: texture_1d<f32>;

[[group(1), binding(3)]]
var r_i32_data: texture_1d<i32>;

fn lambertian_load_albedo(idx: i32) -> vec3<f32> {
    let data_idx = r_world.lambertians.albedo_base_idx + idx;
    return textureLoad(r_vec4_f32_data, data_idx, 0).xyz;
}

fn lambertian_scatter(idx: i32, rng: ptr<function, Xoshiro128Plus>, args: ptr<function, ScatterArgs>, out: ptr<function, ScatterOutput>) -> bool {
    let albedo = lambertian_load_albedo(idx);
    let hit = (*args).hit;
    
    var dir: vec3<f32> = hit.normal + xoshiro128plus_random_unit_sphere_vec3_f32(rng);
    
    if (dot(dir, dir) == 0.0) {
        dir = hit.normal;
    }
    
    *out = ScatterOutput(albedo, Ray(hit.at, dir));
    
    return true;
}

fn metal_load_albedo(idx: i32) -> vec3<f32> {
    let data_idx = r_world.metals.albedo_base_idx + idx;
    return textureLoad(r_vec4_f32_data, data_idx, 0).xyz;
}

fn metal_load_fuzz(idx: i32) -> f32 {
    let data_idx = r_world.metals.fuzz_base_idx + idx;
    return textureLoad(r_f32_data, data_idx, 0).x;
}

fn metal_scatter(idx: i32, rng: ptr<function, Xoshiro128Plus>, args: ptr<function, ScatterArgs>, out: ptr<function, ScatterOutput>) -> bool {
    let normal = (*args).hit.normal;
    let refl = reflect((*args).ray.dir, normal);
    let fuzz = metal_load_fuzz(idx);
    let dir = refl + fuzz * xoshiro128plus_random_unit_ball_vec3_f32(rng);
    
    if (dot(dir, normal) <= 0.0) {
        return false;
    }
    
    let albedo = metal_load_albedo(idx);
    *out = ScatterOutput(albedo, Ray((*args).hit.at, dir));
    
    return true;
}

fn dyn_material_scatter(m: DynMaterial, rng: ptr<function, Xoshiro128Plus>, args: ptr<function, ScatterArgs>, out: ptr<function, ScatterOutput>) -> bool {
    if (m.ty == LAMBERTIAN_MATERIAL_TYPE) {
        return lambertian_scatter(m.idx, rng, args, out);
    } else if (m.ty == METAL_MATERIAL_TYPE) {
        return metal_scatter(m.idx, rng, args, out);
    } else {
        return false;
    }
}

fn sphere_load_center(idx: i32) -> vec3<f32> {
    let data_idx = r_world.spheres.center_base_idx + idx;
    return textureLoad(r_vec4_f32_data, data_idx, 0).xyz;
}

fn sphere_load_radius(idx: i32) -> f32 {
    let data_idx = r_world.spheres.radius_base_idx + idx;
    return textureLoad(r_f32_data, data_idx, 0).x;
}

fn sphere_load_material(idx: i32) -> DynMaterial {
    let type_idx = r_world.spheres.material_ty_base_idx + idx;
    let idx_idx = r_world.spheres.material_idx_base_idx + idx;
    return DynMaterial(textureLoad(r_i32_data, type_idx, 0).x, textureLoad(r_i32_data, idx_idx, 0).x);
}

fn sphere_hit(idx: i32, args: ptr<function, HitArgs>, out: ptr<function, Hit>) -> bool {
    let center = sphere_load_center(idx);
    let radius = sphere_load_radius(idx);
    
    let oc = (*args).ray_norm.orig - center;
    let dir = (*args).ray_norm.dir;
    
    let a = dot(dir, dir);
    let b = dot(oc, dir);
    let c = dot(oc, oc) - radius * radius;
    let d = b * b - a * c;
    
    if (d < 0.0) {
        return false;
    }
    
    let d = sqrt(d);
    let t_min = (*args).t_min;
    let t_sup = (*args).t_sup;

    var t: f32 = (-b - d) / a;
    if (t < t_min || t_sup <= t) {
        t = (-b + d) / a;
    }
    if (t < t_min || t_sup <= t) {
        return false;
    }
    
    let at = ray_normalized_at(&(*args).ray_norm, t);
    var normal: vec3<f32> = (at - center) / radius;
    
    let material = sphere_load_material(idx);
    
    let front_face = dot(normal, dir) <= 0.0;
    
    if (!front_face) {
        normal = -normal;
    }
    
    *out = Hit(at, t, normal, front_face, material);
    
    return true;
}

fn world_hit(args: ptr<function, HitArgs>, out: ptr<function, Hit>) -> bool {
    var temp_args: HitArgs = *args;
    var temp_hit: Hit = hit_nil();
    var result: bool = false;
    
    // Spheres
    for (var i: i32 = 0; i < r_world.spheres.length; i = i + 1) {
        if (sphere_hit(i, &temp_args, &temp_hit)) {
            temp_args.t_sup = temp_hit.t;
            *out = temp_hit;
            result = true;
        }
    }
    
    return result;
}

fn color_sky(y_norm: f32) -> vec3<f32> {
    let t = 0.5 * y_norm + 0.5;
    return mix(vec3<f32>(1.0), vec3<f32>(0.5, 0.7, 1.0), t);
}

fn color_world(ray_norm: Ray, rng: ptr<function, Xoshiro128Plus>) -> vec3<f32> {
    var result: ScatterOutput = ScatterOutput(vec3<f32>(1.0), ray_norm);
    
    for (var i: u32 = r_locals.depth; i > 0u; i = i - 1u) {
        var hit_args: HitArgs = HitArgs(result.ray, 0.001, 1.0e4);
        var hit: Hit = hit_nil();
        
        if (!world_hit(&hit_args, &hit)) {
            return result.attenuation * color_sky(result.ray.dir.y);
        }
        
        let attenuation_prev = result.attenuation;
        var scatter_args: ScatterArgs = ScatterArgs(result.ray, hit);
        if (!dyn_material_scatter(hit.material, rng, &scatter_args, &result)) {
             return vec3<f32>(0.0);
        }

        result.attenuation = attenuation_prev * result.attenuation;
        result.ray.dir = normalize(result.ray.dir);
    }

    return vec3<f32>(0.0);
}

let FOCAL_LENGTH: f32 = 1.0;
let ORIGIN: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let pixel_side = 2.0 / f32(r_locals.shape.y);
    let viewport_base = (in.pixel_pos - 0.5 * vec2<f32>(r_locals.shape)) * pixel_side;

    var color: vec3<f32> = vec3<f32>(0.0);
    var rng: Xoshiro128Plus = xoshiro128plus_load(in.pixel_pos);
    for (var i: u32 = 0u; i < r_locals.sample_count; i = i + 1u) {
        let sample_offset = xoshiro128plus_random_vec2_f32(&rng) * pixel_side;
        let viewport = viewport_base + sample_offset;
        color = color + color_world(Ray(ORIGIN, normalize(vec3<f32>(viewport, -FOCAL_LENGTH) - ORIGIN)), &rng);
    }
    color = color / f32(r_locals.sample_count);
    
    return vec4<f32>(color, 1.0);
}
