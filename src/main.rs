use std::{borrow::Cow, thread, time::Duration};

use bytemuck::{bytes_of, Pod, Zeroable};
use rand::Rng;
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferAddress, BufferDescriptor,
    BufferSize, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, DeviceDescriptor, Maintain, MaintainBase, MapMode, Queue,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, SurfaceConfiguration,
};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod generate;
mod render;

const WIDTH: u32 = 40;
const HEIGHT: u32 = 48;
const WORKGROUP_SIZE: (u32, u32) = (8, 8);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Config {
    width: u32,
    height: u32,
}

struct Life {
    config: Config,
    pipeline: ComputePipeline,
    bind_group: BindGroup,
    input_buffer: Buffer,
    output_buffer: Buffer,
    staging_buffer: Buffer,
    data_size: BufferAddress,
}

impl Life {
    async fn new(device: &Device, config: Config, data: Vec<u32>) -> Self {
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
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Staging buffer"),
            size: buffer_size,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
            ],
        });

        Self {
            config,
            input_buffer,
            output_buffer,
            staging_buffer,
            data_size: buffer_size,
            bind_group: compute_bind_group,
            pipeline: compute_pipeline,
        }
    }
    async fn step(&mut self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Compute shader command encoder"),
        });
        let x_groups = self.config.width / WORKGROUP_SIZE.0;
        let y_groups = self.config.height / WORKGROUP_SIZE.1;

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
        encoder.copy_buffer_to_buffer(
            &self.output_buffer,
            0,
            &self.staging_buffer,
            0,
            self.data_size,
        );

        // Dispatch commands to be executed
        queue.submit(Some(encoder.finish()));
    }
    async fn output(&mut self, device: &Device) -> Vec<u32> {
        let buffer_slice = self.staging_buffer.slice(..);
        let (sender, receiver) = futures_intrusive::channel::shared::oneshot_channel();

        buffer_slice.map_async(MapMode::Read, move |v| sender.send(v).unwrap());
        device.poll(Maintain::Wait);

        if let Some(Ok(())) = receiver.receive().await {
            let data = buffer_slice.get_mapped_range();
            let result = bytemuck::cast_slice(&data).to_vec();
            drop(data);
            self.staging_buffer.unmap();
            result
        } else {
            panic!("Failed to run compute shader!");
        }
    }
}

fn display(data: &[u32], config: Config) {
    for (i, pixel) in data.iter().enumerate() {
        if i as u32 % config.width == 0 {
            println!("|");
        }
        if *pixel == 0 {
            print!("  ");
        } else {
            print!("██")
        }
    }
    println!("|");
    println!("{}|", "--".repeat(config.width as usize));
}

struct State {
    window: Window,
    surface: Surface,
    window_config: SurfaceConfiguration,
    window_size: PhysicalSize<u32>,
    device: Device,
    queue: Queue,
    life: Life,
}

impl State {
    async fn new(window: Window, data: Vec<u32>, config: Config) -> Self {
        // GPU INITIALISATION
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        println!("{:#?}", adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .unwrap();

        // SETTING UNIVERSAL WINDOW STUFF (no move)
        let window_size = window.inner_size();
        let surface = unsafe { instance.create_surface(&window) }.unwrap();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|f| f.describe().srgb)
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let window_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &window_config);

        // SHARED BETWEEN LIFE AND RENDERER HAS TO BE HERE
        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            format: wgpu::TextureFormat::Rgba32Float,
            view_formats: &[],
        });
        let output_texture_view =
            output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // INIT COMPUTE SHADER
        let life = life::Life::new(data, params, &device, &output_texture_view);

        Self {
            window,
            surface,
            window_config,
            window_size,
            device,
            queue,
            life,
        }
    }
    fn window(&self) -> &Window {
        todo!()
    }
    fn update(&mut self) {}
    fn resize(&self, new_size: PhysicalSize<u32>) {
        todo!()
    }
    fn input(&mut self, _: &WindowEvent) -> bool {
        false
    }
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        Ok(())
    }
}

async fn run() {
    // Instantiate handle to the GPU
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions::default())
        .await
        .expect("Failed to acquire adapter");
    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: Some("Device"),
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
            },
            None,
        )
        .await
        .expect("Failed to acquire device and queue");

    let data = generate::gen();
    // Generate data
    let config = Config {
        width: WIDTH,
        height: HEIGHT,
    };
    display(&data, config);

    let mut life = Life::new(&device, config, data).await;

    loop {
        thread::sleep(Duration::from_millis(40));
        life.step(&device, &queue).await;
        let result = life.output(&device).await;
        clearscreen::clear().unwrap();
        display(&result, config);
    }

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let config = Config {
        width: WIDTH,
        height: HEIGHT,
    };
    let data = generate::gen();
    let state = State {};
    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Pressed,
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                },
                            ..
                        } => *control_flow = ControlFlow::Exit,
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                            state.resize(**new_inner_size);
                        }
                        _ => {}
                    }
                }
            }
            Event::RedrawRequested(window_id) if window_id == state.window().id() => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        state.resize(state.window_size)
                    }
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(wgpu::SurfaceError::Timeout) => log::warn!("Surface timeout"),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                state.window().request_redraw();
            }
            _ => {}
        }
    });
}

fn main() {
    pollster::block_on(run());
}
