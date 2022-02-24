struct VertexInput {
    [[location(0)]] position: vec4<f32>;
};

struct VertexOutput {
    [[builtin(position)]] position: vec4<f32>;
    [[location(0)]] uv: vec2<f32>;
};

[[stage(vertex)]]
fn vs_main(
    in: VertexInput,
) -> VertexOutput {
    return VertexOutput(in.position, (in.position.xy + vec2<f32>(1.0)) / 2.0);
}

struct ParamUniform {
    origin: vec3<f32>;
    focal_length: f32;
    image_shape: vec2<f32>;
    viewport_shape: vec2<f32>;
};

[[group(0), binding(0)]]
var<uniform> params: ParamUniform;

struct Sphere {
    center: vec3<f32>;
    radius: f32;
};

let sphere_count: u32 = 2u;

struct Spheres {
    data: array<Sphere, 2>;
};

[[group(1), binding(0)]]
var<uniform> spheres: Spheres;

struct Ray {
    origin: vec3<f32>;
    direction: vec3<f32>;
};

fn ray_at(ray: Ray, t: f32) -> vec3<f32> {
    return ray.origin + t * ray.direction;
}

struct Hit {
    position: vec3<f32>;
    t: f32;
    normal: vec3<f32>;
    front_face: bool;
};

let inf_f32: f32 = 1.0e15;

fn hit_default() -> Hit {
    return Hit(vec3<f32>(0.0), inf_f32, vec3<f32>(0.0, 0.0, 1.0), false);
}

fn hit_set_face_normal(hit: ptr<function, Hit>, ray: Ray) {
    let outward_normal = (*hit).normal;
    let front_face = dot(ray.direction, outward_normal) < 0.0;
    (*hit).front_face = front_face;
    if (front_face) {
        (*hit).normal = outward_normal;
    } else {
        (*hit).normal = -outward_normal;
    }
}

fn sphere_hit(sphere: Sphere, ray: Ray, t_min: f32, t_max: f32, report: ptr<function, Hit>) -> bool {
    let oc = ray.origin - sphere.center;
    let a = dot(ray.direction, ray.direction);
    let b = dot(oc, ray.direction);
    let c = dot(oc, oc) - sphere.radius * sphere.radius;

    let d = b * b - a * c;
    if (d < 0.0) {
        return false;
    }
    let sd = sqrt(d);

    var t: f32 = (-b - sd) / a;
    
    if (t < t_min || t_max < t) {
        t = (-b + sd) / a;

        if (t < t_min || t_max < t) {
            return false;
        }
    }
    
    let p = ray_at(ray, t);
    let n = (p - sphere.center) / sphere.radius;
    *report = Hit(p, t, n, false);
    hit_set_face_normal(report, ray);
    return true;
}

fn any_hit(ray: Ray, t_min: f32, t_max: f32, report: ptr<function, Hit>) -> bool {
    var temp_report: Hit = hit_default();
    var hit_anything: bool = false;
    var closest: f32 = t_max;
    
    for(var i: u32 = 0u; i < sphere_count; i = i + 1u) {
        let sphere = spheres.data[i];
        if (sphere_hit(sphere, ray, t_min, closest, &temp_report)) {
            hit_anything = true;
            closest = temp_report.t;
            *report = temp_report;
        }
    }
    
    return hit_anything;
}

fn ray_color(ray: Ray) -> vec3<f32> {
    var hit: Hit = hit_default();
    
    if (any_hit(ray, 0.0, inf_f32, &hit)) {
        return 0.5 * (hit.normal + vec3<f32>(1.0));
    }

    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return mix(vec3<f32>(1.0), vec3<f32>(0.5, 0.7, 1.0), t);
}

fn ray_from_uv(uv: vec2<f32>) -> Ray {
    return Ray(params.origin, vec3<f32>(params.viewport_shape * (uv - 0.5), -params.focal_length));
}

let SAMPLE: vec2<u32> = vec2<u32>(4u, 4u);

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let sample_subpixel = vec2<f32>(1.0) / (params.image_shape * vec2<f32>(SAMPLE));
    let sample_count = SAMPLE.x * SAMPLE.y;
    var color: vec3<f32> = vec3<f32>(0.0);

    for (var j: u32 = 0u; j < SAMPLE.y; j = j + 1u) {
        for (var i: u32 = 0u; i < SAMPLE.x; i = i + 1u) {
            let ij = vec2<u32>(i, j);
            let uv = in.uv + vec2<f32>(ij) * sample_subpixel;
            color = color + ray_color(ray_from_uv(uv)) / f32(sample_count);
        }
    }
    return vec4<f32>(color, 1.0);
}

