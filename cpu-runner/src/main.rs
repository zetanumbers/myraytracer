#![feature(slice_as_chunks, test, bench_black_box, div_duration)]

#[cfg(test)]
extern crate test;

mod renderer;
mod state;

mod winit {
    pub use winit::{
        dpi::PhysicalSize,
        event::{ElementState, Event, KeyboardInput, StartCause, VirtualKeyCode, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::Window,
    };
}
use parking_lot::FairMutex as Mutex;
use state::State;
use std::{sync::Arc, time};

const SAMPLES_PER_PIXEL: usize = 32;
const ORIGIN: glam::Vec3 = glam::Vec3::ZERO;
const FOCAL_LENGTH: f32 = 1.0;
const UPDATE_RATE: f64 = 2.;
const FRAME_RATE: f64 = 1.;
const MAX_DEPTH: u32 = 50;

fn main() {
    env_logger::init();

    let event_loop = winit::EventLoop::new();
    let window = winit::Window::new(&event_loop).expect("Creating a window");

    let state = Arc::new(State {
        // WARNING: pixels should never outlive window, see https://github.com/parasyte/pixels/issues/238
        pixels: Mutex::new({
            let size = window.inner_size();
            let surface = pixels::SurfaceTexture::new(size.width, size.height, &window);
            pixels::Pixels::new(size.width, size.height, surface).unwrap()
        }),
        window,
        primitives: example_primitives(),
    });
    let mut renderer = renderer::Handle::new(Arc::clone(&state));
    let frame = time::Duration::from_secs_f64(1. / FRAME_RATE);
    let mut next_frame = time::Instant::now() + frame;

    event_loop.run(move |event, _, control_flow| {
        let now = time::Instant::now();
        if next_frame < now {
            state.window.request_redraw();
            next_frame = now + frame;
        }
        *control_flow = if renderer.is_running().unwrap() {
            winit::ControlFlow::WaitUntil(next_frame)
        } else {
            winit::ControlFlow::Wait
        };
        match event {
            winit::Event::RedrawRequested(_) => {
                state.pixels.lock().render().unwrap();
            }
            winit::Event::WindowEvent { event, .. } => match event {
                winit::WindowEvent::Resized(size) => {
                    renderer.break_join().unwrap();

                    let mut pixels = state.pixels.lock();
                    pixels.resize_buffer(size.width, size.height);
                    let (frame_colors, rest) = pixels.get_frame().as_chunks_mut::<4>();
                    assert_eq!(rest, []);
                    frame_colors.fill([0, 0, 0, 255]);

                    pixels.resize_surface(size.width, size.height);
                    drop(pixels);

                    state.window.request_redraw();
                    renderer.restart(Arc::clone(&state)).unwrap();
                }
                winit::WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            state: winit::ElementState::Released,
                            virtual_keycode: Some(winit::VirtualKeyCode::R),
                            ..
                        },
                    is_synthetic: false,
                    ..
                } => {
                    renderer.break_join().unwrap();

                    let mut pixels = state.pixels.lock();
                    let (frame_colors, rest) = pixels.get_frame().as_chunks_mut::<4>();
                    assert_eq!(rest, []);
                    frame_colors.fill([0, 0, 0, 255]);

                    renderer = renderer::Handle::new(Arc::clone(&state));
                    state.window.request_redraw();
                }
                winit::WindowEvent::CloseRequested => *control_flow = winit::ControlFlow::Exit,
                _ => (),
            },
            _ => (),
        }
    })
}

fn example_primitives() -> Vec<raytracer::Primitive> {
    let ground = raytracer::materials::Lambertian {
        albedo: glam::vec3(0.8, 0.8, 0.),
    };
    let center = raytracer::materials::Lambertian {
        albedo: glam::vec3(0.7, 0.3, 0.3),
    };
    let left = raytracer::materials::Metal {
        albedo: glam::vec3(0.8, 0.8, 0.8),
        fuzz: 0.3,
    };
    let right = raytracer::materials::Metal {
        albedo: glam::vec3(0.8, 0.6, 0.2),
        fuzz: 1.0,
    };

    vec![
        raytracer::primitives::Sphere {
            center: glam::vec3(0., -100.5, -1.),
            radius: 100.,
            material: ground.into(),
        }
        .into(),
        raytracer::primitives::Sphere {
            center: glam::vec3(0., 0., -1.),
            radius: 0.5,
            material: center.into(),
        }
        .into(),
        raytracer::primitives::Sphere {
            center: glam::vec3(-1., 0., -1.),
            radius: 0.5,
            material: left.into(),
        }
        .into(),
        raytracer::primitives::Sphere {
            center: glam::vec3(1., 0., -1.),
            radius: 0.5,
            material: right.into(),
        }
        .into(),
    ]
}
