#![feature(slice_as_chunks)]

mod winit {
    pub use winit::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::Window,
    };
}
use glam::{vec2, Vec4};

fn main() {
    let event_loop = winit::EventLoop::new();
    let window = winit::Window::new(&event_loop).expect("Creating a window");
    // WARNING: pixels should never outlive window due to https://github.com/parasyte/pixels/issues/238
    let mut pixels = {
        let size = window.inner_size();
        let surface = pixels::SurfaceTexture::new(size.width, size.height, &window);
        pixels::Pixels::new(size.width, size.height, surface).unwrap()
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::ControlFlow::Wait;
        match event {
            winit::Event::RedrawRequested(_) => {
                let size = window.inner_size();

                let (frame_colors, rest) = pixels.get_frame().as_chunks_mut::<4>();
                assert_eq!(rest, []);
                assert_eq!(
                    frame_colors.len(),
                    size.width as usize * size.height as usize
                );

                let size = vec2(size.width as f32, size.height as f32);
                frame_colors.into_iter().enumerate().for_each(|(i, f)| {
                    let i = i as f32;
                    let xy = vec2(i % size.x, i / size.x);
                    let uv = xy / size;
                    *f = raytracer::pixel(uv)
                        .clamp(Vec4::ZERO, Vec4::ONE)
                        .to_array()
                        .map(|c| (c * 255.) as u8);
                });

                pixels.render().unwrap();
            }
            winit::Event::WindowEvent {
                event: winit::WindowEvent::Resized(size),
                ..
            } => {
                pixels.resize_buffer(size.width, size.height);
                pixels.resize_surface(size.width, size.height);
                window.request_redraw();
            }
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::ControlFlow::Exit,
            _ => (),
        }
    })
}
