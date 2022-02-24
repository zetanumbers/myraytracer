use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::dpi;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Params {
    pub origin: [f32; 3],
    pub focal_length: f32,
    pub image_shape: [f32; 2],
    pub viewport_shape: [f32; 2],
}

impl Params {
    pub fn new(size: dpi::PhysicalSize<u32>) -> Self {
        Params {
            origin: [0., 0., 0.],
            focal_length: 1.0,
            image_shape: [size.width as f32, size.height as f32],
            viewport_shape: [2.0 * size.width as f32 / size.height as f32, 2.0],
        }
    }
}

pub struct ParamUniform {
    layout: wgpu::BindGroupLayout,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ParamUniform {
    pub fn new(device: &wgpu::Device, params: &Params) -> Self {
        let bytes = bytemuck::bytes_of(params);

        let layout_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(bytes.len() as _),
            },
            count: None,
        };

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("param_bind_group_layout"),
            entries: &[layout_entry],
        });

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let entry = wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: None,
            }),
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("param_bind_group"),
            layout: &layout,
            entries: &[entry],
        });

        Self {
            layout,
            buffer,
            bind_group,
        }
    }

    pub fn set(&mut self, queue: &wgpu::Queue, params: &Params) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(params))
    }

    /// Get a reference to the param uniform's layout.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn attach<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, index: u32) {
        rpass.set_bind_group(index, &self.bind_group, &[])
    }
}
