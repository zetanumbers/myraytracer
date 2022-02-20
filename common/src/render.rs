use std::{
    mem,
    os::raw::{c_int, c_void},
    pin::Pin,
    ptr, slice,
};

pub trait Render {
    /// Returns `false` when redraw is obsolete.
    #[must_use]
    fn render(self: Pin<&mut Self>, frame: &mut [u8]) -> bool;
}

pub struct Renderer {
    pub raw: RawRenderer,
}

impl<R> From<Pin<Box<R>>> for Renderer
where
    R: Render + Send + 'static,
{
    fn from(r: Pin<Box<R>>) -> Self {
        pub unsafe extern "C" fn render<R>(data: *mut c_void, frame: *mut u8, len: usize) -> c_int
        where
            R: Render + Send + 'static,
        {
            if Pin::new_unchecked(&mut *(data as *mut R))
                .render(slice::from_raw_parts_mut(frame, len))
            {
                1
            } else {
                0
            }
        }

        pub unsafe extern "C" fn drop_renderer<R>(data: *mut c_void)
        where
            R: Render + Send + 'static,
        {
            Box::from_raw(data.cast::<R>());
        }

        unsafe {
            Renderer::from_raw(RawRenderer::new(
                Box::into_raw(Pin::into_inner_unchecked(r)) as *mut c_void,
                &RawRendererVTable {
                    render: render::<R>,
                    drop: drop_renderer::<R>,
                },
            ))
        }
    }
}

impl Renderer {
    pub unsafe fn from_raw(raw: RawRenderer) -> Self {
        Self { raw }
    }

    pub fn render(&mut self, frame: &mut [u8]) -> bool {
        unsafe { ((*self.raw.vtable).render)(self.raw.data, frame.as_mut_ptr(), frame.len()) != 0 }
    }

    pub fn into_raw(self) -> RawRenderer {
        let mut this = mem::MaybeUninit::new(self);
        let this_mut = unsafe { this.assume_init_mut() };
        let raw = ptr::addr_of_mut!(this_mut.raw);
        drop(this_mut);
        unsafe { raw.read() }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe { ((*self.raw.vtable).drop)(self.raw.data) }
    }
}

#[repr(C)]
pub struct RawRenderer {
    pub data: *mut c_void,
    pub vtable: *const RawRendererVTable,
}

impl RawRenderer {
    pub fn new(data: *mut c_void, vtable: &'static RawRendererVTable) -> Self {
        Self { data, vtable }
    }
}

#[repr(C)]
pub struct RawRendererVTable {
    pub render: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> c_int,
    pub drop: unsafe extern "C" fn(*mut c_void),
}

impl RawRendererVTable {
    pub fn new(
        render: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> c_int,
        drop: unsafe extern "C" fn(*mut c_void),
    ) -> Self {
        Self { render, drop }
    }
}

pub fn render_fn<F>(f: F) -> RenderFn<F>
where
    F: FnMut(&mut [u8]) -> bool + Unpin,
{
    RenderFn(f)
}

pub struct RenderFn<F>(F);

impl<F> Render for RenderFn<F>
where
    F: FnMut(&mut [u8]) -> bool + Unpin,
{
    fn render(mut self: Pin<&mut Self>, frame: &mut [u8]) -> bool {
        (self.0)(frame)
    }
}
