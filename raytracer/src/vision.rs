use crate::{materials::MaterialEnum, primitives::Sphere};
use enum_dispatch::enum_dispatch;
use std::ops;

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: glam::Vec3,
    pub direction: glam::Vec3,
}

impl Ray {
    pub fn new(origin: glam::Vec3, direction: glam::Vec3) -> Self {
        Ray { origin, direction }
    }

    pub fn at(self, t: f32) -> glam::Vec3 {
        self.origin + self.direction * t
    }

    pub fn hit<P>(self, visible: &P, t_r: ops::Range<f32>) -> Option<Hit>
    where
        P: Visible + ?Sized,
    {
        visible.hit_with_ray(self, t_r)
    }

    pub fn hit_collection<'a, I, T>(self, iter: I, mut t_r: ops::Range<f32>) -> Option<Hit<'a>>
    where
        I: IntoIterator<Item = &'a T>,
        T: Visible + ?Sized + 'a,
    {
        let mut out = None;
        for v in iter {
            if let Some(hit) = self.hit(v, t_r.clone()) {
                out = Some(hit);
                t_r.end = hit.t;
            }
        }
        out
    }
}

#[enum_dispatch]
pub trait Visible {
    fn hit_with_ray(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit<'_>>;
}

impl<T: Visible + ?Sized> Visible for &'_ T {
    fn hit_with_ray(&self, ray: Ray, t_r: ops::Range<f32>) -> Option<Hit<'_>> {
        Visible::hit_with_ray(*self, ray, t_r)
    }
}

#[derive(Clone, Copy)]
pub struct Hit<'a> {
    pub at: glam::Vec3,
    pub t: f32,
    pub normal: glam::Vec3,
    pub face: Face,
    pub material: &'a MaterialEnum,
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
    pub fn correct_face(mut self, ray: Ray) -> Self {
        if self.normal.dot(ray.direction) > 0. {
            self.normal = -self.normal;
            self.face = -self.face;
        }
        self
    }
}

#[enum_dispatch(Visible)]
#[derive(Clone, Copy)]
pub enum Primitive {
    Sphere,
}
