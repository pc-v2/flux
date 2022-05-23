use flux::settings::{ColorScheme, Mode, Noise, Settings};
use flux::Flux;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::{ControlFlow, EventLoop};
use glutin::window::Window;
use glutin::PossiblyCurrent;
use semver::Version;
use std::rc::Rc;

fn main() {
    let settings = Settings {
        version: Version::new(2, 0, 0),
        mode: Mode::Normal,
        viscosity: 5.0,
        velocity_dissipation: 0.0,
        starting_pressure: 0.0,
        fluid_size: 128,
        fluid_simulation_frame_rate: 60.0,
        diffusion_iterations: 4,
        pressure_iterations: 20,
        color_scheme: ColorScheme::Peacock,
        line_length: 400.0,
        line_width: 7.0,
        line_begin_offset: 0.5,
        line_variance: 0.47,
        grid_spacing: 14,
        view_scale: 1.6,
        noise_channels: vec![
            Noise {
                scale: 2.3,
                multiplier: 1.0,
                offset_increment: 1.0 / 1024.0,
            },
            Noise {
                scale: 13.8,
                multiplier: 0.7,
                offset_increment: 1.0 / 1024.0,
            },
            Noise {
                scale: 27.6,
                multiplier: 0.5,
                offset_increment: 1.0 / 1024.0,
            },
        ],
    };

    let logical_size = glutin::dpi::LogicalSize::new(1200, 900);
    let (context, window, event_loop) =
        get_rendering_context(logical_size.width, logical_size.height);
    let physical_size = logical_size.to_physical(window.window().scale_factor());

    let context = Rc::new(context);
    let mut flux = Flux::new(
        &context,
        logical_size.width,
        logical_size.height,
        physical_size.width,
        physical_size.height,
        &Rc::new(settings),
    )
    .unwrap();

    let start = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        let next_frame_time =
            std::time::Instant::now() + std::time::Duration::from_nanos(16_666_667);
        *control_flow = glutin::event_loop::ControlFlow::WaitUntil(next_frame_time);

        match event {
            Event::LoopDestroyed => {
                return;
            }

            Event::MainEventsCleared => {
                window.window().request_redraw();
            }

            Event::RedrawRequested(_) => {
                flux.animate(start.elapsed().as_millis() as f32);
                window.swap_buffers().unwrap();
            }

            Event::WindowEvent { ref event, .. } => match event {
                WindowEvent::Resized(physical_size) => {
                    window.resize(*physical_size);
                    let logical_size = physical_size.to_logical(window.window().scale_factor());
                    flux.resize(
                        logical_size.width,
                        logical_size.height,
                        physical_size.width,
                        physical_size.height,
                    );
                }
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            _ => (),
        }
    });
}

pub fn get_rendering_context(
    width: u32,
    height: u32,
) -> (
    glow::Context,
    glutin::ContextWrapper<PossiblyCurrent, Window>,
    EventLoop<()>,
) {
    let event_loop = glutin::event_loop::EventLoop::new();
    let window_builder = glutin::window::WindowBuilder::new()
        .with_title("Flux")
        .with_decorations(true)
        .with_resizable(true)
        .with_inner_size(glutin::dpi::LogicalSize::new(width, height));
    let window = glutin::ContextBuilder::new()
        .with_vsync(true)
        .with_multisampling(0)
        .with_double_buffer(Some(true))
        .build_windowed(window_builder, &event_loop)
        .unwrap();
    let window = unsafe { window.make_current().unwrap() };

    let gl =
        unsafe { glow::Context::from_loader_function(|s| window.get_proc_address(s) as *const _) };

    (gl, window, event_loop)
}
