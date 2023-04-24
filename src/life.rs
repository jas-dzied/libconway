use std::borrow::Cow;

use bytemuck::{Pod, Zeroable};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, Buffer, BufferAddress,
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
    ComputePipeline, ComputePipelineDescriptor, Device, Queue, ShaderModuleDescriptor,
    ShaderSource, TextureView,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Config {
    pub width: u32,
    pub height: u32,
}

pub struct Life {
    config: Config,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    input_buffer: Buffer,
    output_buffer: Buffer,
    data_size: BufferAddress,
}

impl Life {
    pub async fn new(
        device: &Device,
        texture_view: &TextureView,
        config: Config,
        data: Vec<u32>,
    ) -> Self {
        let data_slice_size = data.len() * std::mem::size_of::<u32>();
        let buffer_size = data_slice_size as BufferAddress;

        // Instantiate compute shader buffers
        let compute_shader = Cow::Borrowed(include_str!("../shaders/life.wgsl"));
        let compute_shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Compute shader module"),
            source: ShaderSource::Wgsl(compute_shader),
        });
        let input_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Input buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        });
        let output_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Output buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let config_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Config buffer"),
            contents: bytemuck::bytes_of(&config),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create compute shader bind group
        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Compute shader pipeline"),
            layout: None,
            module: &compute_shader_module,
            entry_point: "main",
        });
        let compute_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute shader bind group"),
            layout: &compute_pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: config_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&texture_view),
                },
            ],
        });

        Self {
            config,
            input_buffer,
            output_buffer,
            data_size: buffer_size,
            bind_group: compute_bind_group,
            pipeline: compute_pipeline,
        }
    }
    pub async fn step(&mut self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Compute shader command encoder"),
        });
        let x_groups = self.config.width / crate::WORKGROUP_SIZE.0;
        let y_groups = self.config.height / crate::WORKGROUP_SIZE.1;

        // Simulate a single step of life
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Compute shader pass"),
        });
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.bind_group, &[]);
        compute_pass.insert_debug_marker("Compute shader runtime");
        compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
        drop(compute_pass);

        // Copies data from the output buffer to the input buffer
        encoder.copy_buffer_to_buffer(
            &self.output_buffer,
            0,
            &self.input_buffer,
            0,
            self.data_size,
        );

        // Dispatch commands to be executed
        queue.submit(Some(encoder.finish()));
    }
}
