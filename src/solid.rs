use bytemuck::{Pod, Zeroable};

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Sphere {
    pub center: [f32; 3],
    pub radius: f32,
}

pub struct Solids {
    layout: wgpu::BindGroupLayout,
    _buffer: wgpu::Buffer,
    _spheres_range: std::ops::Range<wgpu::BufferAddress>,
    bind_group: wgpu::BindGroup,
}

impl Solids {
    pub fn new(device: &wgpu::Device, spheres: &[Sphere]) -> Self {
        let spheres_bytes: &[u8] = bytemuck::cast_slice(&spheres);
        let spheres_size = spheres_bytes.len() as wgpu::BufferAddress;
        let spheres_range = 0..spheres_size;

        let sphere_layout_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(wgpu::BufferSize::new(spheres_size).unwrap()),
            },
            count: None,
        };

        let size = spheres_size;

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("solids"),
            entries: &[sphere_layout_entry],
        });

        let buffer = {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("solids"),
                size,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: true,
            });

            // Max align satisfies align(16)
            buffer
                .slice(spheres_range.clone())
                .get_mapped_range_mut()
                .copy_from_slice(spheres_bytes);

            buffer.unmap();

            buffer
        };

        let sphere_entry = wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: spheres_range.start,
                size: Some(wgpu::BufferSize::new(spheres_size).unwrap()),
            }),
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("solids"),
            layout: &layout,
            entries: &[sphere_entry],
        });

        Self {
            layout,
            _buffer: buffer,
            _spheres_range: spheres_range,
            bind_group,
        }
    }

    /// Get a reference to the solids's layout.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn attach<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, index: u32) {
        rpass.set_bind_group(index, &self.bind_group, &[])
    }
}
