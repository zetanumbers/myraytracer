use crate::{primitives::Hit, ray::Ray};
use rand_distr::Distribution;
use rand_pcg::Pcg32;

pub trait Material {
    fn scatter(&self, rng: &mut Pcg32, ray: Ray, hit: Hit) -> Option<Scatter>;
}

#[derive(Clone, Copy)]
pub struct Scatter {
    pub attenuation: glam::Vec3,
    pub ray: Ray,
}

#[derive(Clone, Copy)]
pub struct Lambertian {
    pub albedo: glam::Vec3,
}

impl Material for Lambertian {
    fn scatter(&self, rng: &mut Pcg32, _: Ray, hit: Hit) -> Option<Scatter> {
        let direction = hit.normal + glam::Vec3::from(rand_distr::UnitSphere.sample(rng));
        Some(Scatter {
            ray: Ray {
                origin: hit.at,
                direction,
            },
            attenuation: self.albedo,
        })
    }
}
