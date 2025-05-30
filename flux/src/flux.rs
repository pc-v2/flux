use crate::{grid, render, rng, settings};
use settings::Settings;

use std::sync::Arc;
use std::sync::Mutex;

// The time at which the animation timer will reset to zero.
const MAX_ELAPSED_TIME: f32 = 1000.0;
const MAX_FRAME_TIME: f32 = 1.0 / 10.0;

pub struct Flux {
    settings: Arc<Settings>,
    logical_size: wgpu::Extent3d,
    physical_size: wgpu::Extent3d,

    grid: grid::Grid,
    fluid: render::fluid::Context,
    pub lines: render::lines::Context,
    noise_generator: render::noise::NoiseGenerator,
    debug_texture: render::texture::Context,

    pub color_image: Arc<Mutex<Option<image::RgbaImage>>>,

    // A timestamp in milliseconds. Either host or video time.
    last_timestamp: f64,

    // A local animation timer in seconds that resets at MAX_ELAPSED_TIME.
    elapsed_time: f32,

    fluid_frame_time: f32,
}

impl Flux {
    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, settings: &Arc<Settings>) {
        self.settings = Arc::clone(settings);
        self.fluid
            .update(device, queue, self.grid.scaling_ratio, &self.settings);
        self.noise_generator.update(&self.settings);
        self.lines
            .update(device, queue, self.logical_size, &self.grid, &self.settings);
    }

    pub fn sample_colors_from_image(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: &image::RgbaImage,
    ) {
        let texture_view = render::color::load_color_texture(device, queue, image);
        self.sample_colors_from_texture_view(device, queue, texture_view);
    }

    pub fn sample_colors_from_texture_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_view: wgpu::TextureView,
    ) {
        self.lines
            .update_color_bindings(device, queue, Some(texture_view), None);
    }

    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Arc<Settings>,
    ) -> Result<Flux, String> {
        log::info!("✨ Initialising Flux");

        rng::init_from_seed(&settings.seed);

        let logical_size = wgpu::Extent3d {
            width: logical_width,
            height: logical_height,
            depth_or_array_layers: 1,
        };
        let physical_size = wgpu::Extent3d {
            width: physical_width,
            height: physical_height,
            depth_or_array_layers: 1,
        };

        log::info!("📐 Logical size: {}x{}", logical_width, logical_height);
        log::info!("📏 Physical size: {}x{}", physical_width, physical_height);

        let grid = grid::Grid::new(logical_width, logical_height, settings.grid_spacing);

        let fluid = render::fluid::Context::new(device, queue, grid.scaling_ratio, settings);

        let lines = render::lines::Context::new(
            device,
            queue,
            swapchain_format,
            logical_size,
            &grid,
            settings,
        );

        let mut noise_generator_builder = render::noise::NoiseGeneratorBuilder::new(
            2 * settings.fluid_size,
            grid.scaling_ratio,
            settings,
        );
        settings.noise_channels.iter().for_each(|channel| {
            noise_generator_builder.add_channel(channel);
        });
        let noise_generator = noise_generator_builder.build(device, queue);

        let debug_texture = render::texture::Context::new(
            device,
            swapchain_format,
            &[
                ("fluid", fluid.get_velocity_texture_view()),
                ("noise", noise_generator.get_noise_texture_view()),
                ("pressure", fluid.get_pressure_texture_view()),
                ("divergence", fluid.get_divergence_texture_view()),
            ],
        );

        Ok(Flux {
            settings: Arc::clone(settings),
            logical_size,
            physical_size,

            fluid,
            grid,
            lines,
            noise_generator,
            debug_texture,
            color_image: Arc::new(Mutex::new(None)),

            last_timestamp: 0.0,
            elapsed_time: 0.0,

            fluid_frame_time: 0.0,
        })
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
    ) {
        let grid = grid::Grid::new(logical_width, logical_height, self.settings.grid_spacing);

        // TODO: fetch line state from GPU and resample for new grid
        let logical_size = wgpu::Extent3d {
            width: logical_width,
            height: logical_height,
            depth_or_array_layers: 1,
        };
        let physical_size = wgpu::Extent3d {
            width: physical_width,
            height: physical_height,
            depth_or_array_layers: 1,
        };

        self.lines
            .resize(device, queue, logical_size, &grid, &self.settings);

        self.grid = grid;
        self.logical_size = logical_size;
        self.physical_size = physical_size;

        // self.fluid.resize(device, self.grid.scaling_ratio);
        self.noise_generator.resize(
            device,
            2 * self.settings.fluid_size,
            self.grid.scaling_ratio,
        );
    }

    pub fn animate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_viewport: Option<render::ScreenViewport>,
        timestamp: f64,
    ) {
        self.compute(device, queue, encoder, timestamp);
        self.render(device, queue, encoder, view, screen_viewport);
    }

    pub fn compute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        timestamp: f64,
    ) {
        // The delta time in seconds
        let timestep = f32::min(
            MAX_FRAME_TIME,
            0.001 * (timestamp - self.last_timestamp) as f32,
        );

        self.last_timestamp = timestamp;
        self.elapsed_time += timestep;
        self.fluid_frame_time += timestep;

        // Reset animation timers to avoid precision issues
        let timer_overflow = self.elapsed_time - MAX_ELAPSED_TIME;
        if timer_overflow >= 0.0 {
            self.elapsed_time = timer_overflow;
        }

        while self.fluid_frame_time >= self.settings.fluid_timestep {
            self.noise_generator
                .update_buffers(queue, self.settings.fluid_timestep);

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::compute"),
                timestamp_writes: None,
            });

            self.noise_generator.generate(&mut cpass);

            self.fluid.advect_forward(queue, &mut cpass);
            self.fluid.advect_reverse(queue, &mut cpass);
            self.fluid.adjust_advection(&mut cpass);
            self.fluid.diffuse(&mut cpass);

            let velocity_bind_group = self.fluid.get_write_velocity_bind_group();
            self.noise_generator.inject_noise_into(
                &mut cpass,
                velocity_bind_group,
                self.fluid.get_fluid_size(),
            );

            self.fluid.calculate_divergence(&mut cpass);
            self.fluid.solve_pressure(queue, &mut cpass);
            self.fluid.subtract_gradient(&mut cpass);

            self.fluid_frame_time -= self.settings.fluid_timestep;
        }

        {
            self.lines
                .tick_line_uniforms(device, queue, timestep, self.elapsed_time);

            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("flux::place_lines"),
                timestamp_writes: None,
            });

            self.lines
                .place_lines(&mut cpass, self.fluid.get_read_velocity_bind_group());
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_viewport: Option<render::ScreenViewport>,
    ) {
        encoder.push_debug_group("render lines");

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("flux::render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            use settings::Mode::*;
            match &self.settings.mode {
                Normal => {
                    let view_transform = screen_viewport
                        .map(|ref sv| {
                            render::ViewTransform::from_screen_viewport(&self.physical_size, sv)
                        })
                        .unwrap_or_default();
                    self.lines.set_view_transform(queue, view_transform);
                    self.lines.draw_lines(&mut rpass);
                    self.lines.draw_endpoints(&mut rpass);
                }
                DebugNoise => {
                    self.debug_texture.draw_texture(device, &mut rpass, "noise");
                }
                DebugFluid => {
                    self.debug_texture.draw_texture(device, &mut rpass, "fluid");
                }
                DebugPressure => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "pressure");
                }
                DebugDivergence => {
                    self.debug_texture
                        .draw_texture(device, &mut rpass, "divergence");
                }
            };
        }

        encoder.pop_debug_group();
    }
}

// #[derive(Debug)]
// pub enum Problem {
//     ReadSettings(String),
//     ReadImage(std::io::Error),
//     DecodeColorTexture(image::ImageError),
//     Render(render::Problem),
// }
//
// impl fmt::Display for Problem {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Problem::ReadSettings(msg) => write!(f, "{}", msg),
//             Problem::ReadImage(msg) => write!(f, "{}", msg),
//             Problem::DecodeColorTexture(msg) => write!(f, "Failed to decode image: {}", msg),
//             Problem::Render(render_msg) => write!(f, "{}", render_msg),
//         }
//     }
// }
