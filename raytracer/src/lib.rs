#![feature(result_option_inspect)]

pub mod materials;
pub mod primitives;
mod vision;

use std::ops;

use rand_pcg::Pcg32;

pub use crate::{
    materials::{Material, MaterialEnum},
    vision::{Primitive, Ray, Visible},
};

#[derive(Clone, Copy)]
pub struct Input<'a> {
    pub primitives: &'a [Primitive],
}

pub fn color(input: &Input<'_>, rng: &mut Pcg32, ray: &Ray, depth: u32) -> glam::Vec3 {
    if depth <= 0 {
        return glam::vec3(0., 0., 0.);
    }

    let init_t_range = ops::Range {
        start: 0.001,
        end: f32::INFINITY,
    };

    ray.hit_collection(input.primitives, &init_t_range)
        .map(|hit| {
            hit.material
                .scatter(rng, ray, &hit)
                .map(|s| s.attenuation * color(input, rng, &s.ray, depth - 1))
                .unwrap_or(glam::Vec3::ZERO)
        })
        .unwrap_or_else(|| {
            let t = 0.5 * (ray.direction.normalize_or_zero().y + 1.);
            glam::Vec3::ONE.lerp(glam::vec3(0.5, 0.7, 1.0), t)
        })
}
