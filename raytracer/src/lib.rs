use bytemuck::{Pod, Zeroable};
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use std::{borrow::Cow, future::Future, mem, num::NonZeroU64, pin::Pin, sync::Arc, task};
use waker::AppEventDispatchWaker;
use wgpu::util::DeviceExt;
use winit::{
    dpi,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

mod waker;

pub use winit;

#[derive(Clone, Copy, Debug)]
pub struct Args {
    pub width: u32,
    pub height: u32,
    pub samples_per_frame: u32,
    pub ray_depth: u32,
    pub max_framebuffer_weight: f32,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            width: 0,
            height: 0,
            ray_depth: 50,
            samples_per_frame: 1,
            max_framebuffer_weight: 1.0,
        }
    }
}

pub struct PlatformArgs {
    // TODO: Use better cfg condition like web-sys?
    #[cfg(target_arch = "wasm32")]
    pub canvas: web_sys::HtmlCanvasElement,
}

#[derive(Copy, Clone, Debug)]
pub enum AppEvent {
    InitializeWake,
}

type AppEventDispatch = EventLoopProxy<AppEvent>;

#[derive(Default)]
enum AppState {
    /// A temporary state to move out fields
    #[default]
    Taken,
    Uninitialized {
        args: Args,
        platform: PlatformArgs,
        dispatch: AppEventDispatch,
    },
    Initializing {
        waker: task::Waker,
        future: Pin<Box<dyn Future<Output = State>>>,
    },
    Running {
        state: State,
    },
    Closed,
}

pub struct App {
    state: AppState,
}

impl App {
    pub fn new(event_loop: &EventLoop<AppEvent>, args: Args, platform: PlatformArgs) -> Self {
        App {
            state: AppState::Uninitialized {
                args,
                platform,
                dispatch: event_loop.create_proxy(),
            },
        }
    }

    fn state_as_str(&self) -> &'static str {
        match self.state {
            AppState::Uninitialized { .. } => "uninitialized",
            AppState::Taken => "taken",
            AppState::Initializing { .. } => "initializing",
            AppState::Running { .. } => "running",
            AppState::Closed => "closed",
        }
    }
}

impl winit::application::ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_variables))]
        let AppState::Uninitialized {
            platform,
            mut args,
            dispatch,
        } = mem::take(&mut self.state)
        else {
            return;
        };

        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes().with_resizable(false);

        'set_size: {
            [args.width, args.height] = match args {
                Args {
                    width: 0,
                    height: 0,
                    ..
                } => break 'set_size,
                Args {
                    width: side,
                    height: 0,
                    ..
                }
                | Args {
                    width: 0,
                    height: side,
                    ..
                } => [side; 2],
                Args { width, height, .. } => [width, height],
            };

            attrs = attrs.with_inner_size(dpi::LogicalSize::<u32>::from([args.width, args.height]));
        }

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs = attrs
                .with_canvas(Some(platform.canvas))
                .with_prevent_default(false)
                .with_focusable(false);
        }

        let window = event_loop
            .create_window(attrs)
            .expect("failed to create a window");

        if args.width == 0 && args.height == 0 {
            dpi::PhysicalSize {
                width: args.width,
                height: args.height,
            } = window.inner_size();
        }

        let future = Box::pin(async move { State::new(window, &args).await });

        let waker = AppEventDispatchWaker::new(dispatch, AppEvent::InitializeWake).into_waker();
        // Start initialization
        waker.wake_by_ref();

        self.state = AppState::Initializing { waker, future }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: AppEvent) {
        log::debug!("User event: {event:?}");
        match event {
            AppEvent::InitializeWake => {
                if let AppState::Initializing { waker, future } = &mut self.state {
                    let mut cx = task::Context::from_waker(waker);
                    if let task::Poll::Ready(state) = future.as_mut().poll(&mut cx) {
                        state.request_redraw();
                        self.state = AppState::Running { state };
                    }
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.state = AppState::Closed;
                log::info!("Close requested, exiting...");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => match &mut self.state {
                AppState::Initializing { .. } | AppState::Closed => (),
                AppState::Running { state } => {
                    state.redraw();
                    state.request_redraw();
                }
                AppState::Taken | AppState::Uninitialized { .. } => {
                    panic!("Requested redraw but app is {}", self.state_as_str())
                }
            },
            _ => (),
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        // TODO
    }
}

struct State {
    base: Base,
    subject: Subject,
    object: Object,
    framebuffers: DoubleFramebuffers,
    raytrace_glue: RaytraceGlue,
    framebuffer_glue: FramebufferGlue,
    sample_count: u32,
}

impl State {
    async fn new(window: Window, args: &Args) -> Self {
        let base = Base::new(window, args).await;
        let subject = Subject::new(&base, args);
        let object = Object::new(&base, args);
        let framebuffers = DoubleFramebuffers::new(&base, args);
        let raytrace_glue = RaytraceGlue::new(&base, &subject, &object, &framebuffers);
        let framebuffer_glue = FramebufferGlue::new(&base, &subject, &framebuffers);

        State {
            base,
            subject,
            object,
            framebuffers,
            raytrace_glue,
            framebuffer_glue,
            sample_count: 0,
        }
    }

    #[inline]
    fn request_redraw(&self) {
        self.base.window.request_redraw()
    }

    fn redraw(&mut self) {
        let mut encoder = self
            .base
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.framebuffers.target.fb_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.raytrace_glue.render_pipeline);
            rpass.set_bind_group(0, &self.subject.bind_group, &[]);
            rpass.set_bind_group(1, &self.object.bind_group, &[]);
            rpass.set_bind_group(2, &self.framebuffers.secondary.bind_group, &[]);
            rpass.set_vertex_buffer(0, self.raytrace_glue.vertices.slice(..));
            rpass.draw(0..4, 0..1);
        }

        let frame = self.base.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.framebuffer_glue.render_pipeline);
            rpass.set_bind_group(0, &self.subject.bind_group, &[]);
            rpass.set_bind_group(1, &self.framebuffers.target.bind_group, &[]);
            rpass.set_vertex_buffer(0, self.framebuffer_glue.vertices.slice(..));
            rpass.draw(0..4, 0..1);
        }

        self.base.queue.submit(Some(encoder.finish()));
        frame.present();

        self.framebuffers.swap();
        self.sample_count = self.sample_count.saturating_add(1);
        self.subject.locals.framebuffer_weight = self
            .framebuffers
            .max_framebuffer_weight
            .min(self.sample_count as f32 / (self.sample_count + 1) as f32);
        self.subject.locals.rng_shuffle = rand::thread_rng().gen();
        self.subject.update_locals_buffer(&self.base);
    }
}

struct Base {
    window: Arc<Window>,
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    swapchain_format: wgpu::TextureFormat,
}

impl Base {
    async fn new(window: Window, args: &Args) -> Self {
        let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..<_>::default()
        });

        let window = Arc::new(window);
        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("failed to create a surface");

        let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface))
            .await
            .expect("No suitable GPU adapters found on the system!");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Requesting device");

        let surface_config = surface
            .get_default_config(&adapter, args.width, args.height)
            .expect("failed to get default surface config");
        let swapchain_format = surface_config.format;

        surface.configure(&device, &surface_config);

        Base {
            window,
            _instance: instance,
            surface,
            _adapter: adapter,
            device,
            queue,
            surface_config,
            swapchain_format,
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Zeroable, Pod)]
struct Locals {
    shape: [u32; 2],
    samples_per_frame: u32,
    ray_depth: u32,
    rng_shuffle: [u32; 4],
    framebuffer_weight: f32,
    _padding: [u32; 3],
}

struct Subject {
    locals: Locals,
    locals_buffer: wgpu::Buffer,
    _rng: wgpu::Texture,
    _rng_view: wgpu::TextureView,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Subject {
    fn new(base: &Base, args: &Args) -> Self {
        let mut seed_rng = rand_xoshiro::SplitMix64::from_entropy();

        let rng_texture_data: Vec<[u32; 4]> = std::iter::repeat_with(|| seed_rng.gen())
            .filter(|s| s != &[0; 4])
            .take(args.width as usize * args.height as usize)
            .collect();

        let rng = base.device.create_texture_with_data(
            &base.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: args.width,
                    height: args.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[wgpu::TextureFormat::Rgba32Uint],
            },
            <_>::default(),
            bytemuck::cast_slice(&rng_texture_data),
        );

        drop(rng_texture_data);

        let locals = Locals {
            shape: [args.width, args.height],
            samples_per_frame: args.samples_per_frame,
            rng_shuffle: [0; 4],
            ray_depth: args.ray_depth,
            framebuffer_weight: 0.0,
            _padding: [0; 3],
        };
        let locals_buffer = base
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&locals),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group_layout =
            base.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Uint,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });

        let rng_view = rng.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(wgpu::TextureFormat::Rgba32Uint),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            ..<_>::default()
        });
        let bind_group = base.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("subject"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &locals_buffer,
                        offset: 0,
                        size: Some(NonZeroU64::new(mem::size_of::<Locals>() as u64).unwrap()),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&rng_view),
                },
            ],
        });

        Self {
            locals,
            locals_buffer,
            _rng: rng,
            _rng_view: rng_view,
            bind_group_layout,
            bind_group,
        }
    }

    fn update_locals_buffer(&mut self, base: &Base) {
        base.queue
            .write_buffer(&self.locals_buffer, 0, bytemuck::bytes_of(&self.locals));
    }
}

struct DoubleFramebuffers {
    bind_group_layout: wgpu::BindGroupLayout,
    max_framebuffer_weight: f32,
    target: Framebuffer,
    secondary: Framebuffer,
}

impl DoubleFramebuffers {
    fn new(base: &Base, args: &Args) -> Self {
        let bind_group_layout =
            base.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    }],
                });
        DoubleFramebuffers {
            target: Framebuffer::new(base, args, &bind_group_layout),
            secondary: Framebuffer::new(base, args, &bind_group_layout),
            bind_group_layout,
            max_framebuffer_weight: args.max_framebuffer_weight,
        }
    }

    fn swap(&mut self) {
        mem::swap(&mut self.target, &mut self.secondary)
    }
}

struct Framebuffer {
    _fb: wgpu::Texture,
    fb_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl Framebuffer {
    fn new(base: &Base, args: &Args, bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let fb = base.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: args.width,
                height: args.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: base.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[base.surface_config.format],
        });

        let fb_view = fb.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(base.surface_config.format),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            ..<_>::default()
        });
        let bind_group = base.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("framebuffer"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&fb_view),
            }],
        });

        Self {
            _fb: fb,
            fb_view,
            bind_group,
        }
    }
}

struct Object {
    _base_indices: wgpu::Buffer,
    _data_vec4_f32: wgpu::Texture,
    _data_f32: wgpu::Texture,
    _data_i32: wgpu::Texture,
    _view_vec4_f32: wgpu::TextureView,
    _view_f32: wgpu::TextureView,
    _view_i32: wgpu::TextureView,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Object {
    fn new(base: &Base, _: &Args) -> Self {
        mod api {
            #[derive(Clone, Copy)]
            pub struct Lambertian {
                pub albedo: [f32; 3],
            }

            #[derive(Clone, Copy)]
            pub struct Metal {
                pub albedo: [f32; 3],
                pub fuzz: f32,
            }

            #[derive(Clone, Copy)]
            pub enum DynMaterial {
                Lambertian(Lambertian),
                Metal(Metal),
            }

            #[derive(Clone, Copy)]
            pub struct Sphere {
                pub center: [f32; 3],
                pub radius: f32,
                pub material: DynMaterial,
            }

            pub struct World {
                pub spheres: Vec<Sphere>,
            }
        }

        mod raw {
            use bytemuck::{Pod, Zeroable};

            #[repr(i32)]
            pub enum MaterialTy {
                Lambertian = 1,
                Metal = 2,
            }

            #[repr(C)]
            #[derive(Clone, Copy, Zeroable, Pod)]
            pub struct SphereRange {
                pub center_base_idx: i32,
                pub radius_base_idx: i32,
                pub material_ty_base_idx: i32,
                pub material_idx_base_idx: i32,
                pub length: i32,
                pub _padding: [i32; 3],
            }

            #[repr(C)]
            #[derive(Clone, Copy, Zeroable, Pod)]
            pub struct LambertianRange {
                pub albedo_base_idx: i32,
                pub length: i32,
                pub _padding: [i32; 2],
            }

            #[repr(C)]
            #[derive(Clone, Copy, Zeroable, Pod)]
            pub struct MetalRange {
                pub albedo_base_idx: i32,
                pub fuzz_base_idx: i32,
                pub length: i32,
                pub _padding: i32,
            }

            #[repr(C)]
            #[derive(Clone, Copy, Zeroable, Pod)]
            pub struct World {
                pub spheres: SphereRange,
                pub lambertians: LambertianRange,
                pub metals: MetalRange,
            }
        }

        let world = api::World {
            spheres: vec![
                api::Sphere {
                    center: [0., -100.5, -1.],
                    radius: 100.,
                    material: api::DynMaterial::Lambertian(api::Lambertian {
                        albedo: [0.8, 0.8, 0.],
                    }),
                },
                api::Sphere {
                    center: [0., 0., -1.],
                    radius: 0.5,
                    material: api::DynMaterial::Lambertian(api::Lambertian {
                        albedo: [0.7, 0.3, 0.3],
                    }),
                },
                api::Sphere {
                    center: [-1., 0., -1.],
                    radius: 0.5,
                    material: api::DynMaterial::Metal(api::Metal {
                        albedo: [0.8, 0.8, 0.8],
                        fuzz: 0.3,
                    }),
                },
                api::Sphere {
                    center: [1., 0., -1.],
                    radius: 0.5,
                    material: api::DynMaterial::Metal(api::Metal {
                        albedo: [0.8, 0.6, 0.2],
                        fuzz: 1.0,
                    }),
                },
            ],
        };

        let mut sphere_centers = Vec::new();
        let mut sphere_radiuses = Vec::new();
        let mut sphere_material_idxs = Vec::new();
        let mut sphere_material_tys = Vec::new();

        let mut lambertian_albedos = Vec::new();
        let mut metal_albedos = Vec::new();
        let mut metal_fuzzes = Vec::new();

        for sphere in &world.spheres {
            sphere_centers.push(sphere.center);
            sphere_radiuses.push(sphere.radius);
            let material_idx;
            match sphere.material {
                api::DynMaterial::Lambertian(api::Lambertian { albedo }) => {
                    sphere_material_tys.push(raw::MaterialTy::Lambertian as i32);
                    material_idx = lambertian_albedos.len() as i32;
                    lambertian_albedos.push(albedo);
                }
                api::DynMaterial::Metal(api::Metal { albedo, fuzz }) => {
                    sphere_material_tys.push(raw::MaterialTy::Metal as i32);
                    material_idx = metal_albedos.len() as i32;
                    metal_albedos.push(albedo);
                    metal_fuzzes.push(fuzz);
                }
            };
            sphere_material_idxs.push(material_idx);
        }

        let lambertian_length = lambertian_albedos.len() as i32;
        let metal_length = metal_albedos.len() as i32;
        let spheres_length = world.spheres.len() as i32;

        let mut vec4_f32_data = Vec::new();
        let mut f32_data = Vec::new();
        let mut i32_data = Vec::new();

        fn push<T, I>(data: &mut Vec<T>, extend: I) -> i32
        where
            I: IntoIterator<Item = T>,
        {
            let base_idx = data.len();
            data.extend(extend);
            base_idx as i32
        }

        let raw_world = raw::World {
            spheres: raw::SphereRange {
                center_base_idx: push(
                    &mut vec4_f32_data,
                    sphere_centers.into_iter().map(|[x, y, z]| [x, y, z, 1.0]),
                ),
                radius_base_idx: push(&mut f32_data, sphere_radiuses),
                material_ty_base_idx: push(&mut i32_data, sphere_material_tys),
                material_idx_base_idx: push(&mut i32_data, sphere_material_idxs),
                length: spheres_length,
                _padding: <_>::zeroed(),
            },
            lambertians: raw::LambertianRange {
                albedo_base_idx: push(
                    &mut vec4_f32_data,
                    lambertian_albedos
                        .into_iter()
                        .map(|[x, y, z]| [x, y, z, 1.0]),
                ),
                length: lambertian_length,
                _padding: <_>::zeroed(),
            },
            metals: raw::MetalRange {
                albedo_base_idx: push(
                    &mut vec4_f32_data,
                    metal_albedos.into_iter().map(|[x, y, z]| [x, y, z, 1.0]),
                ),
                fuzz_base_idx: push(&mut f32_data, metal_fuzzes),
                length: metal_length,
                _padding: <_>::zeroed(),
            },
        };

        let base_indices = base
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world uniform buffer"),
                contents: bytemuck::bytes_of(&raw_world),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let vec4_f32_data_tex_desc = wgpu::TextureDescriptor {
            label: Some("vec4_f32_data"),
            size: wgpu::Extent3d {
                width: vec4_f32_data.len() as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            format: wgpu::TextureFormat::Rgba32Float,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba32Float],
        };

        let data_vec4_f32 = base.device.create_texture_with_data(
            &base.queue,
            &vec4_f32_data_tex_desc,
            <_>::default(),
            bytemuck::cast_slice(&vec4_f32_data),
        );

        let data_f32 = base.device.create_texture_with_data(
            &base.queue,
            &wgpu::TextureDescriptor {
                label: Some("f32_data"),
                size: wgpu::Extent3d {
                    width: f32_data.len() as u32,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                format: wgpu::TextureFormat::R32Float,
                view_formats: &[wgpu::TextureFormat::R32Float],
                ..vec4_f32_data_tex_desc
            },
            <_>::default(),
            bytemuck::cast_slice(&f32_data),
        );

        let data_i32 = base.device.create_texture_with_data(
            &base.queue,
            &wgpu::TextureDescriptor {
                label: Some("i32_data"),
                size: wgpu::Extent3d {
                    width: i32_data.len() as u32,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                format: wgpu::TextureFormat::R32Sint,
                view_formats: &[wgpu::TextureFormat::R32Sint],
                ..vec4_f32_data_tex_desc
            },
            <_>::default(),
            bytemuck::cast_slice(&i32_data),
        );

        let bind_group_layout =
            base.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("world"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: Some(
                                    NonZeroU64::new(mem::size_of::<raw::World>() as u64).unwrap(),
                                ),
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D1,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D1,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Sint,
                                view_dimension: wgpu::TextureViewDimension::D1,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });

        let view_vec4_f32 = data_vec4_f32.create_view(&wgpu::TextureViewDescriptor {
            label: Some("vec4_f32_data"),
            format: Some(wgpu::TextureFormat::Rgba32Float),
            dimension: Some(wgpu::TextureViewDimension::D1),
            aspect: wgpu::TextureAspect::All,
            ..<_>::default()
        });
        let view_f32 = data_f32.create_view(&wgpu::TextureViewDescriptor {
            label: Some("f32_data"),
            format: Some(wgpu::TextureFormat::R32Float),
            dimension: Some(wgpu::TextureViewDimension::D1),
            aspect: wgpu::TextureAspect::All,
            ..<_>::default()
        });
        let view_i32 = data_i32.create_view(&wgpu::TextureViewDescriptor {
            label: Some("i32_data"),
            format: Some(wgpu::TextureFormat::R32Sint),
            dimension: Some(wgpu::TextureViewDimension::D1),
            aspect: wgpu::TextureAspect::All,
            ..<_>::default()
        });

        let bind_group = base.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("objective state"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &base_indices,
                        offset: 0,
                        size: Some(NonZeroU64::new(mem::size_of::<raw::World>() as u64).unwrap()),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view_vec4_f32),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view_f32),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view_i32),
                },
            ],
        });

        Object {
            _base_indices: base_indices,
            _data_vec4_f32: data_vec4_f32,
            _data_f32: data_f32,
            _data_i32: data_i32,
            _view_vec4_f32: view_vec4_f32,
            _view_f32: view_f32,
            _view_i32: view_i32,
            bind_group_layout,
            bind_group,
        }
    }
}

struct RaytraceGlue {
    _shader: wgpu::ShaderModule,
    vertices: wgpu::Buffer,
    _pipeline_layout: wgpu::PipelineLayout,
    render_pipeline: wgpu::RenderPipeline,
}

impl RaytraceGlue {
    fn new(
        base: &Base,
        subject: &Subject,
        object: &Object,
        framebuffers: &DoubleFramebuffers,
    ) -> Self {
        const VERTEX_DATA: &[[f32; 2]] = &[[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]];

        let vertices = base
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(VERTEX_DATA),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let shader = base
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        let pipeline_layout = base
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    &subject.bind_group_layout,
                    &object.bind_group_layout,
                    &framebuffers.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = base
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[vertex_buffer_layout],
                    compilation_options: <_>::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[wgpu::ColorTargetState {
                        format: base.swapchain_format,
                        blend: None,
                        write_mask: <_>::default(),
                    }
                    .into()],
                    compilation_options: <_>::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..<_>::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        RaytraceGlue {
            _shader: shader,
            vertices,
            _pipeline_layout: pipeline_layout,
            render_pipeline,
        }
    }
}

struct FramebufferGlue {
    _shader: wgpu::ShaderModule,
    vertices: wgpu::Buffer,
    _pipeline_layout: wgpu::PipelineLayout,
    render_pipeline: wgpu::RenderPipeline,
}

impl FramebufferGlue {
    // TODO: share code
    fn new(base: &Base, subject: &Subject, framebuffers: &DoubleFramebuffers) -> Self {
        const VERTEX_DATA: &[[f32; 2]] = &[[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]];

        let vertices = base
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(VERTEX_DATA),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        };

        let shader = base
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "sample_framebuffer.wgsl"
                ))),
            });

        let pipeline_layout = base
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&subject.bind_group_layout, &framebuffers.bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = base
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[vertex_buffer_layout],
                    compilation_options: <_>::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[wgpu::ColorTargetState {
                        format: base.swapchain_format,
                        blend: None,
                        write_mask: <_>::default(),
                    }
                    .into()],
                    compilation_options: <_>::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..<_>::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        FramebufferGlue {
            _shader: shader,
            vertices,
            _pipeline_layout: pipeline_layout,
            render_pipeline,
        }
    }
}
