use std::ops;

#[derive(Clone, Copy, Debug)]
pub struct Zero;

#[derive(Clone, Copy)]
pub struct NonZero<T>(T);

impl<T> NonZero<T> {
    pub unsafe fn new_unchecked(v: T) -> Self {
        NonZero(v)
    }

    pub fn as_ref(&self) -> NonZero<&T> {
        NonZero(&self.0)
    }

    pub fn get(self) -> T {
        self.0
    }
}

impl<T> NonZero<&'_ T> {
    pub fn copied(self) -> NonZero<T>
    where
        T: Copy,
    {
        NonZero(*self.0)
    }

    pub fn cloned(self) -> NonZero<T>
    where
        T: Clone,
    {
        NonZero(self.0.clone())
    }
}

impl<T: ops::Neg> ops::Neg for NonZero<T> {
    type Output = NonZero<T::Output>;

    fn neg(self) -> Self::Output {
        unsafe { NonZero::new_unchecked(-self.get()) }
    }
}

#[derive(Clone, Copy)]
pub struct Normalized<T>(NonZero<T>);

impl<T> Normalized<T> {
    pub unsafe fn new_unchecked(v: T) -> Self {
        Normalized(NonZero(v))
    }

    pub fn as_ref(&self) -> Normalized<&T> {
        Normalized(self.0.as_ref())
    }

    pub fn get(self) -> T {
        self.0.get()
    }

    pub fn into_non_zero(self) -> NonZero<T> {
        self.0
    }
}

impl<T> Normalized<&'_ T> {
    pub fn copied(self) -> Normalized<T>
    where
        T: Copy,
    {
        unsafe { Normalized::new_unchecked(*self.get()) }
    }

    pub fn cloned(self) -> Normalized<T>
    where
        T: Clone,
    {
        unsafe { Normalized::new_unchecked(self.get().clone()) }
    }
}

impl<T: ops::Neg> ops::Neg for Normalized<T> {
    type Output = Normalized<T::Output>;

    fn neg(self) -> Self::Output {
        unsafe { Normalized::new_unchecked(-self.get()) }
    }
}

impl<T> Normalized<NonZero<T>> {
    pub fn flatten(self) -> Normalized<T> {
        Normalized(self.get())
    }
}

pub trait Normalize: Sized {
    type Base;

    fn normalize(self) -> Normalized<Self::Base>;
}

impl From<Zero> for glam::Vec3 {
    fn from(Zero: Zero) -> Self {
        glam::Vec3::ZERO
    }
}

impl From<Zero> for &'static glam::Vec3 {
    fn from(Zero: Zero) -> Self {
        &glam::Vec3::ZERO
    }
}

impl AsRef<glam::Vec3> for Zero {
    fn as_ref(&self) -> &glam::Vec3 {
        &glam::Vec3::ZERO
    }
}

impl TryFrom<glam::Vec3> for NonZero<glam::Vec3> {
    type Error = Zero;

    fn try_from(value: glam::Vec3) -> Result<Self, Self::Error> {
        if &value == &glam::Vec3::ZERO {
            Err(Zero)
        } else {
            Ok(unsafe { NonZero::new_unchecked(value) })
        }
    }
}

impl From<NonZero<glam::Vec3>> for glam::Vec3 {
    fn from(value: NonZero<glam::Vec3>) -> Self {
        value.get()
    }
}

impl From<Normalized<glam::Vec3>> for glam::Vec3 {
    fn from(v: Normalized<glam::Vec3>) -> Self {
        v.get()
    }
}

impl From<Normalized<glam::Vec3>> for NonZero<glam::Vec3> {
    fn from(v: Normalized<glam::Vec3>) -> Self {
        v.into_non_zero()
    }
}

impl Normalize for NonZero<glam::Vec3> {
    type Base = glam::Vec3;

    fn normalize(self) -> Normalized<Self::Base> {
        unsafe { Normalized::new_unchecked(self.get().normalize()) }
    }
}
