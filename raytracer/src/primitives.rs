use crate::{materials::Material, ray::Ray};
use std::ops;

pub trait Visible {
    fn hit_with_ray(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit<'_>>;
}

#[derive(Clone, Copy)]
pub struct Hit<'a> {
    pub at: glam::Vec3,
    pub t: f32,
    pub normal: glam::Vec3,
    pub face: Face,
    pub material: &'a dyn Material,
}

#[derive(Clone, Copy)]
pub enum Face {
    Front,
    Back,
}

impl ops::Neg for Face {
    type Output = Self;

    fn neg(self) -> Self {
        match self {
            Face::Front => Face::Back,
            Face::Back => Face::Front,
        }
    }
}

impl Hit<'_> {
    fn correct_face(mut self, ray: Ray) -> Self {
        if self.normal.dot(ray.direction) > 0. {
            self.normal = -self.normal;
            self.face = -self.face;
        }
        self
    }
}

pub struct Sphere<M: Material + ?Sized> {
    pub center: glam::Vec3,
    pub radius: f32,
    pub material: M,
}

impl<'a> Sphere<dyn Material + 'a> {
    fn hit_with_ray_impl(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit> {
        let oc = ray.origin - self.center;
        let a = ray.direction.length_squared();
        let b = oc.dot(ray.direction);
        let c = oc.length_squared() - self.radius.powi(2);
        let d = b.powi(2) - a * c;

        let t = (d >= 0.)
            .then(|| (-b - d.sqrt()) / a)
            .filter(|t| t_r.contains(t))?;
        let at = ray.at(t);
        let normal = (at - self.center) / self.radius;

        Some(
            Hit {
                t,
                at,
                normal,
                face: Face::Front,
                material: &self.material,
            }
            .correct_face(ray),
        )
    }
}

impl<M: Material> Visible for Sphere<M> {
    fn hit_with_ray(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit> {
        Sphere::<dyn Material>::hit_with_ray_impl(self, ray, t_r)
    }
}

impl<T: Visible + ?Sized> Visible for &'_ T {
    fn hit_with_ray(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit<'_>> {
        Visible::hit_with_ray(*self, ray, t_r)
    }
}
