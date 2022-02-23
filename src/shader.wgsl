struct ParamUniform {
    origin: vec4<f32>;
    image_shape: vec2<f32>;
    viewport_shape: vec2<f32>;
    focal_length: f32;
    _padding0: f32;
    _padding1: f32;
    _padding2: f32;
};

[[group(0), binding(0)]]
var<uniform> params: ParamUniform;

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

struct Ray {
    origin: vec3<f32>;
    direction: vec3<f32>;
};

fn ray_at(ray: Ray, t: f32) -> vec3<f32> {
    return ray.origin + t * ray.direction;
}

fn hit_sphere(center: vec3<f32>, radius: f32, ray: Ray) -> f32 {
    let oc = ray.origin - center;
    let a = dot(ray.direction, ray.direction);
    let hb = dot(oc, ray.direction);
    let c = dot(oc, oc) - radius * radius;
    let discriminant = hb * hb - a * c;
    if (discriminant >= 0.0) {
        return (-hb - sqrt(discriminant)) / a;
    } else {
        return -1.0;
    }
}

fn ray_color(ray: Ray) -> vec3<f32> {
    let center = vec3<f32>(0.0, 0.0, -1.0);
    let radius = 0.5;
    let t = hit_sphere(center, radius, ray);
    if (t > 0.0) {
        let normal = normalize(ray_at(ray, t) - center);
        return 0.5 * (normal + vec3<f32>(1.0));
    }
    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return mix(vec3<f32>(1.0), vec3<f32>(0.5, 0.7, 1.0), t);
}

[[stage(fragment)]]
fn fs_main(in: VertexOutput) -> [[location(0)]] vec4<f32> {
    let lower_left_corner = params.origin.xyz + vec3<f32>(-params.viewport_shape / 2.0, -params.focal_length);
    let ray = Ray(params.origin.xyz, lower_left_corner + vec3<f32>(params.viewport_shape * in.uv, 0.0));
    return vec4<f32>(ray_color(ray), 1.0);
}

