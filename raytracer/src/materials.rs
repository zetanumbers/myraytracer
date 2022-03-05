use crate::vision;
use enum_dispatch::enum_dispatch;
use rand_distr::Distribution;

#[enum_dispatch]
pub trait Material {
    fn scatter(
        &self,
        rng: &mut rand_pcg::Pcg32,
        ray: vision::Ray,
        hit: vision::Hit,
    ) -> Option<Scatter>;
}

#[derive(Clone, Copy)]
pub struct Scatter {
    pub attenuation: glam::Vec3,
    pub ray: vision::Ray,
}

#[derive(Clone, Copy)]
pub struct Lambertian {
    pub albedo: glam::Vec3,
}

impl Material for Lambertian {
    fn scatter(
        &self,
        rng: &mut rand_pcg::Pcg32,
        _: vision::Ray,
        hit: vision::Hit,
    ) -> Option<Scatter> {
        let mut direction = hit.normal + glam::Vec3::from(rand_distr::UnitSphere.sample(rng));

        if direction.length_squared() == 0. {
            direction = hit.normal
        }

        Some(Scatter {
            ray: vision::Ray {
                origin: hit.at,
                direction,
            },
            attenuation: self.albedo,
        })
    }
}

#[enum_dispatch(Material)]
#[derive(Clone, Copy)]
pub enum MaterialEnum {
    Lambertian,
}
