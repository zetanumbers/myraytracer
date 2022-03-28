mod window;

use bytemuck::{Pod, Zeroable};
use rand::Rng;
use rand_xoshiro::rand_core::SeedableRng;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    mem,
    num::NonZeroU64,
    rc::Rc,
    task::{Poll, Waker},
};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::HtmlButtonElement;
use wgpu::util::DeviceExt;

#[derive(serde::Deserialize, Clone, Copy, Debug)]
#[serde(default)]
struct Args {
    width: u32,
    height: u32,
    sample_count: u32,
    log_level: log::Level,
    ray_depth: u32,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            width: 300,
            height: 150,
            ray_depth: 8,
            sample_count: 32,
            log_level: log::Level::Error,
        }
    }
}

#[wasm_bindgen(start)]
pub async fn main() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let query = web_sys::window().unwrap().location().search().unwrap();
    let query = Some(query.as_str())
        .filter(|q| q.is_empty())
        .or_else(|| query.strip_prefix('?'))
        .unwrap();
    let args: Args = serde_urlencoded::from_str(&query).expect("Parsing querry string");
    console_log::init_with_level(args.log_level).expect("Initializing logger");
    log::debug!("Parsed args from query: {args:?}");
    let Args {
        width,
        height,
        sample_count,
        ray_depth,
        ..
    } = args;

    let window = window::CanvasWindow::new(width, height)?;

    log::info!("Intializing the surface...");

    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance = wgpu::Instance::new(backends);
    let surface = unsafe { instance.create_surface(&window) };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .expect("No adapter found");

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

    #[rustfmt::skip]
    let vertices: &[f32] = &[
        -1.0, -1.0, 
        -1.0, 1.0, 
        1.0, -1.0,
        1.0, 1.0,
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(vertices),
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

    let mut seed_rng = rand_xoshiro::SplitMix64::from_entropy();

    let rand_tex_data: Vec<[u32; 4]> = std::iter::from_fn(|| Some(seed_rng.gen()))
        .filter(|s| s != &[0; 4])
        .take((width * height) as usize)
        .collect();

    let rand_tex = device.create_texture_with_data(
        &queue,
        &wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        },
        bytemuck::cast_slice(&rand_tex_data),
    );

    drop(rand_tex_data);

    let rand_tex_view = rand_tex.create_view(&wgpu::TextureViewDescriptor {
        label: None,
        format: Some(wgpu::TextureFormat::Rgba32Uint),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::All,
        ..<_>::default()
    });

    #[repr(C, align(8))]
    #[derive(Clone, Copy, Zeroable, Pod)]
    struct Locals {
        shape: [u32; 2],
        sample_count: u32,
        ray_depth: u32,
    }

    let locals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&Locals {
            shape: [width, height],
            sample_count,
            ray_depth,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
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
                resource: wgpu::BindingResource::TextureView(&rand_tex_view),
            },
        ],
    });

    let (
        world_bind_group_layout,
        world_bind_group,
        _world_buffer,
        _vec4_f32_data_tex,
        _f32_data_tex,
        _i32_data_tex,
    ) = {
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

        let world_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

        let vec4_f32_data_tex = device.create_texture_with_data(
            &queue,
            &vec4_f32_data_tex_desc,
            bytemuck::cast_slice(&vec4_f32_data),
        );

        let f32_data_tex = device.create_texture_with_data(
            &queue,
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

        let i32_data_tex = device.create_texture_with_data(
            &queue,
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

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &world_buffer,
                        offset: 0,
                        size: Some(NonZeroU64::new(mem::size_of::<raw::World>() as u64).unwrap()),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&vec4_f32_data_tex.create_view(
                        &wgpu::TextureViewDescriptor {
                            label: Some("vec4_f32_data"),
                            format: Some(wgpu::TextureFormat::Rgba32Float),
                            dimension: Some(wgpu::TextureViewDimension::D1),
                            aspect: wgpu::TextureAspect::All,
                            ..<_>::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&f32_data_tex.create_view(
                        &wgpu::TextureViewDescriptor {
                            label: Some("f32_data"),
                            format: Some(wgpu::TextureFormat::R32Float),
                            dimension: Some(wgpu::TextureViewDimension::D1),
                            aspect: wgpu::TextureAspect::All,
                            ..<_>::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&i32_data_tex.create_view(
                        &wgpu::TextureViewDescriptor {
                            label: Some("i32_data"),
                            format: Some(wgpu::TextureFormat::R32Sint),
                            dimension: Some(wgpu::TextureViewDimension::D1),
                            aspect: wgpu::TextureAspect::All,
                            ..<_>::default()
                        },
                    )),
                },
            ],
        });

        (
            bind_group_layout,
            bind_group,
            world_buffer,
            vec4_f32_data_tex,
            f32_data_tex,
            i32_data_tex,
        )
    };

    let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout, &world_bind_group_layout],
        push_constant_ranges: &[],
    });

    let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            targets: &[swapchain_format.into()],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..<_>::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: args.width,
        height: args.height,
        present_mode: wgpu::PresentMode::Mailbox,
    };

    surface.configure(&device, &config);

    log::info!("Configuration complete");

    let mut click = Click::new("Draw")?;

    loop {
        (&mut click).await;

        log::info!("Starting to draw...");

        let frame = surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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
            rpass.set_pipeline(&render_pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.set_bind_group(1, &world_bind_group, &[]);
            rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
            rpass.draw(0..4, 0..1);
        }

        queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

struct Click {
    _button: HtmlButtonElement,
    _onclick: wasm_bindgen::closure::Closure<dyn FnMut()>,
    shared: Rc<ClickShared>,
}

struct ClickShared {
    waker: RefCell<Waker>,
    clicked: Cell<bool>,
}

impl Click {
    fn new(text: &str) -> Result<Self, JsValue> {
        let document = web_sys::window().unwrap().document().unwrap();
        let button: HtmlButtonElement = document.create_element("button")?.unchecked_into();
        button.set_inner_text(text);
        document.body().unwrap().append_child(&button)?;

        let shared = Rc::new(ClickShared {
            waker: RefCell::new(futures_task::noop_waker()),
            clicked: Cell::new(false),
        });
        let onclick = wasm_bindgen::closure::Closure::wrap({
            let shared = Rc::clone(&shared);
            Box::new(move || {
                shared.waker.borrow().wake_by_ref();
                shared.clicked.set(true)
            }) as Box<dyn FnMut()>
        });
        button.set_onclick(Some(onclick.as_ref().unchecked_ref()));

        Ok(Click {
            _button: button,
            _onclick: onclick,
            shared,
        })
    }
}

impl std::future::Future for Click {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut futures_task::Context<'_>,
    ) -> futures_task::Poll<Self::Output> {
        let mut waker = self.shared.waker.borrow_mut();
        let new_waker = cx.waker();
        if !waker.will_wake(new_waker) {
            *waker = new_waker.clone();
        }

        if self.shared.clicked.replace(false) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
