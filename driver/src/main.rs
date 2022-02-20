mod renderer_module;

use pixels as px;
use renderer_module::RendererPlugin;
mod wnt {
    pub use winit::{
        dpi::{LogicalSize, PhysicalSize},
        event::{self, Event},
        event_loop::{ControlFlow, EventLoop, EventLoopProxy},
        window::{Fullscreen, Window, WindowBuilder},
    };
}

use std::{env, path::PathBuf};

fn main() {
    env_logger::init();

    let event_loop = wnt::EventLoop::new();
    let window = wnt::WindowBuilder::new()
        .with_title("Hello Pixels")
        .build(&event_loop)
        .unwrap();

    let size = window.inner_size();
    let mut pixels = {
        let surface_texture = px::SurfaceTexture::new(size.width, size.height, &window);
        px::PixelsBuilder::new(size.width, size.height, surface_texture)
            .build()
            .expect("Pixels instantiation")
    };

    let renderer = PathBuf::from(
        env::args_os()
            .nth(1)
            .expect("Expected renderer's dynamic library path as a first argument"),
    );

    let mut renderer = RendererPlugin::new(&renderer);

    #[allow(unused_variables)]
    let size = ();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = wnt::ControlFlow::Poll;

        log::trace!("Got winnit event: {event:?}");

        match event {
            wnt::Event::RedrawRequested(_) => pixels.render().expect("Rendering pixels"),
            wnt::Event::NewEvents(wnt::event::StartCause::Init) => unsafe {
                renderer.load(window.inner_size())
            },
            wnt::Event::NewEvents(wnt::event::StartCause::Poll) => {
                if renderer.changed() {
                    unsafe { renderer.load(window.inner_size()) };
                }

                if renderer.render(pixels.get_frame()) {
                    window.request_redraw()
                }
            }
            wnt::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {
                unsafe { renderer.load(size) };
                pixels.resize_surface(size.width, size.height);
                pixels.resize_buffer(size.width, size.height);
            }
            wnt::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => *control_flow = wnt::ControlFlow::Exit,
            _ => (),
        }
    })
}
