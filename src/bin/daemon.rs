use std::{env, io, process};

use ipc_channel::ipc;
use raytracer::prelude::*;

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

    let mut renderer = Renderer::spawn(size);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = wnt::ControlFlow::Poll;
        match event {
            wnt::Event::RedrawRequested(_) => pixels.render().expect("Rendering pixels"),
            wnt::Event::NewEvents(wnt::event::StartCause::Poll) => {
                let frame = pixels.get_frame();
                let size = window.inner_size();
                while let Some(cmd) = renderer.recv() {
                    match cmd {
                        raytracer::Command::Set { pos, color } => {
                            let color_base = (pos[1] * size.width as usize + pos[0]) * 4;
                            let pixel = &mut frame[color_base..][..4];
                            pixel[..3].copy_from_slice(&color);
                            pixel[3] = 255;
                        }
                        raytracer::Command::Redraw => window.request_redraw(),
                        raytracer::Command::Nop => (),
                    }
                }
            }
            wnt::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {
                renderer = Renderer::spawn(size);
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

struct Renderer {
    process: process::Child,
    receiver: ipc::IpcReceiver<raytracer::Command>,
}

impl Renderer {
    fn spawn(size: wnt::PhysicalSize<u32>) -> Self {
        let mut args = env::args();
        let (server, ipc_name) = ipc::IpcOneShotServer::new().expect("Initializing ipc server");
        let init_args = raytracer::RendererInitArgs {
            size: size.into(),
            ipc_name,
        };
        let process = process::Command::new(args.nth(1).expect("Renderer command not found"))
            .args(args)
            .arg(init_args.serialize())
            .spawn()
            .expect("Spawning renderer process");
        let (receiver, nop) = server.accept().expect("Accepting ipc connection");
        assert_eq!(nop, raytracer::Command::Nop);

        Renderer { process, receiver }
    }

    fn recv(&mut self) -> Option<raytracer::Command> {
        self.receiver.try_recv().map_or_else(
            |e| match e {
                ipc::TryRecvError::Empty => None,
                ipc::TryRecvError::IpcError(ipc::IpcError::Disconnected) => None,
                e => panic!("Trying to recieve next command: {e:?}"),
            },
            Some,
        )
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.process
            .kill()
            .or_else(|e| match e.kind() {
                io::ErrorKind::InvalidInput => Ok(()),
                e => Err(e),
            })
            .expect("Killing renderer")
    }
}
