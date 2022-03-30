use bytemuck::{Pod, Zeroable};
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use std::{borrow::Cow, mem, num::NonZeroU64};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, Debug)]
pub struct Args {
    pub width: u32,
    pub height: u32,
    pub sample_count: u32,
    pub ray_depth: u32,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            width: 400,
            height: 225,
            ray_depth: 50,
            sample_count: 100,
        }
    }
}

pub struct PlatformArgs {
    #[cfg(target_arch = "wasm32")]
    pub canvas: web_sys::HtmlCanvasElement,
}

#[allow(unused_variables)]
pub async fn run(args: Args, platform: PlatformArgs) -> ! {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_resizable(false)
        .with_inner_size(winit::dpi::PhysicalSize::<u32>::from([
            args.width,
            args.height,
        ]));
    #[cfg(target_arch = "wasm32")]
    let window =
        winit::platform::web::WindowBuilderExtWebSys::with_canvas(window, Some(platform.canvas));
    let window = window.build(&event_loop).unwrap();

    let mut state = State::new(window, &args).await;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = winit::event_loop::ControlFlow::Wait;
        match event {
            winit::event::Event::WindowEvent {
                window_id: _,
                event: winit::event::WindowEvent::CloseRequested,
            } => {
                log::info!("Close requested, exiting...");
                *control_flow = winit::event_loop::ControlFlow::Exit;
            }
            winit::event::Event::RedrawRequested(_) => state.redraw(),
            _ => (),
        }
    })
}

struct State {
    base: Base,
    subject: Subject,
    object: Object,
    glue: Glue,
}

impl State {
    async fn new(window: winit::window::Window, args: &Args) -> Self {
        let base = Base::new(window, args).await;
        let subject = Subject::new(&base, args);
        let object = Object::new(&base, args);
        let glue = Glue::new(&base, &subject, &object);

        State {
            base,
            subject,
            object,
            glue,
        }
    }

    fn redraw(&mut self) {
        let frame = self.base.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .base
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.glue.render_pipeline);
            rpass.set_bind_group(0, &self.subject.bind_group, &[]);
            rpass.set_bind_group(1, &self.object.bind_group, &[]);
            rpass.set_vertex_buffer(0, self.glue.vertices.slice(..));
            rpass.draw(0..4, 0..1);
        }

        self.base.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

struct Base {
    window: winit::window::Window,
    instance: wgpu::Instance,
    surface: wgpu::Surface,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    swapchain_format: wgpu::TextureFormat,
}

impl Base {
    async fn new(window: winit::window::Window, args: &Args) -> Self {
        let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
        let instance = wgpu::Instance::new(backends);
        let surface = unsafe { instance.create_surface(&window) };

        let adapter =
            wgpu::util::initialize_adapter_from_env_or_default(&instance, backends, Some(&surface))
                .await
                .expect("No suitable GPU adapters found on the system!");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Requesting device");

        let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: args.width,
            height: args.height,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        surface.configure(&device, &surface_config);

        Base {
            window,
            instance,
            surface,
            adapter,
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
    sample_count: u32,
    ray_depth: u32,
}

struct Subject {
    locals: Locals,
    locals_buffer: wgpu::Buffer,
    rng: wgpu::Texture,
    rng_view: wgpu::TextureView,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Subject {
    fn new(base: &Base, args: &Args) -> Self {
        let mut seed_rng = rand_xoshiro::SplitMix64::from_entropy();

        let rng_texture_data: Vec<[u32; 4]> = std::iter::from_fn(|| Some(seed_rng.gen()))
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
            },
            bytemuck::cast_slice(&rng_texture_data),
        );

        drop(rng_texture_data);

        let locals = Locals {
            shape: [args.width, args.height],
            sample_count: args.sample_count,
            ray_depth: args.ray_depth,
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
            rng,
            rng_view,
            bind_group_layout,
            bind_group,
        }
    }
}

struct Object {
    base_indices: wgpu::Buffer,
    data_vec4_f32: wgpu::Texture,
    data_f32: wgpu::Texture,
    data_i32: wgpu::Texture,
    view_vec4_f32: wgpu::TextureView,
    view_f32: wgpu::TextureView,
    view_i32: wgpu::TextureView,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl Object {
    fn new(base: &Base, args: &Args) -> Self {
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
        };

        let data_vec4_f32 = base.device.create_texture_with_data(
            &base.queue,
            &vec4_f32_data_tex_desc,
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
                ..vec4_f32_data_tex_desc
            },
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
                ..vec4_f32_data_tex_desc
            },
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
            base_indices,
            data_vec4_f32,
            data_f32,
            data_i32,
            view_vec4_f32,
            view_f32,
            view_i32,
            bind_group_layout,
            bind_group,
        }
    }
}

struct Glue {
    shader: wgpu::ShaderModule,
    vertices: wgpu::Buffer,
    pipeline_layout: wgpu::PipelineLayout,
    render_pipeline: wgpu::RenderPipeline,
}

impl Glue {
    fn new(base: &Base, subject: &Subject, object: &Object) -> Self {
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
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        let pipeline_layout = base
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&subject.bind_group_layout, &object.bind_group_layout],
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
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[base.swapchain_format.into()],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    ..<_>::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Glue {
            shader,
            vertices,
            pipeline_layout,
            render_pipeline,
        }
    }
}
