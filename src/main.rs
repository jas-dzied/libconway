use std::time::Instant;

use wgpu::{Device, Queue, Surface, SurfaceConfiguration};
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod generate;
mod life;
mod render;

use generate::Generator;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const WORKGROUP_SIZE: (u32, u32) = (5, 5);

struct State {
    window: Window,
    surface: Surface,
    window_config: SurfaceConfiguration,
    window_size: PhysicalSize<u32>,
    device: Device,
    queue: Queue,
    life: life::Life,
    renderer: render::Renderer,
}

impl State {
    async fn new(window: Window, data: Vec<u32>, config: life::Config) -> Self {
        // GPU INITIALISATION
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
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
        let life = life::Life::new(&device, &output_texture_view, config, data).await;

        let renderer = render::Renderer::new(&device, &window_config, &output_texture_view);

        Self {
            window,
            surface,
            window_config,
            window_size,
            device,
            queue,
            life,
            renderer,
        }
    }
    fn window(&self) -> &Window {
        &self.window
    }
    fn update(&mut self) {
        let start = Instant::now();
        pollster::block_on(self.life.step(&self.device, &self.queue));
        let elapsed = start.elapsed();
        println!("Update took {}ms", elapsed.as_micros() as f32 / 1000.0);
    }
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.window_size = new_size;
            self.window_config.width = new_size.width;
            self.window_config.height = new_size.height;
            self.surface.configure(&self.device, &self.window_config);
        }
    }
    fn input(&mut self, _: &WindowEvent) -> bool {
        false
    }
    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let start = Instant::now();
        let res = self
            .renderer
            .render(&self.surface, &self.device, &self.queue);
        let elapsed = start.elapsed();
        println!("Render took {}ms", elapsed.as_micros() as f32 / 1000.0);
        res
    }
}

async fn run() {
    let config = life::Config {
        width: WIDTH,
        height: HEIGHT,
    };
    let data = generate::Plaintext {
        source: "patterns/breeder_1.cells",
        x_offset: 10,
        y_offset: 740,
    }
    .generate(&config);

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut state = State::new(window, data, config).await;
    event_loop.run(move |event, _, control_flow| match event {
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
            state.window().request_redraw();
        }
        _ => {}
    });
}

fn main() {
    pollster::block_on(run());
}
