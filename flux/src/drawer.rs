use crate::{data, render, settings};
use render::{
    Buffer, Context, Framebuffer, Uniform, UniformBlock, UniformValue, VertexArrayObject,
    VertexBufferLayout,
};
use settings::Settings;

use bytemuck::{Pod, Zeroable};
use crevice::std140::AsStd140;
use glow::HasContext;
use std::rc::Rc;

pub struct Drawer {
    context: Context,
    settings: Rc<Settings>,

    pub grid: Grid,

    logical_width: u32,
    logical_height: u32,
    physical_width: u32,
    physical_height: u32,

    basepoint_buffer: Buffer,
    line_state_buffers: render::DoubleTransformFeedback,
    #[allow(unused)]
    line_vertices: Buffer,
    #[allow(unused)]
    plane_vertices: Buffer,

    place_lines_buffers: Vec<VertexArrayObject>,
    draw_lines_buffers: Vec<VertexArrayObject>,
    draw_endpoints_buffers: Vec<VertexArrayObject>,
    draw_texture_buffer: VertexArrayObject,

    line_uniforms: UniformBlock<LineUniforms>,
    color_texture: Option<render::Framebuffer>,

    place_lines_pass: render::Program,
    draw_lines_pass: render::Program,
    draw_endpoints_pass: render::Program,
    draw_texture_pass: render::Program,
}

impl Drawer {
    pub fn new(
        context: &Context,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
        settings: &Rc<Settings>,
    ) -> Result<Self, render::Problem> {
        let (logical_width, logical_height) = clamp_logical_size(logical_width, logical_height);
        log::debug!("Physical size: {}x{}px", physical_width, physical_height);
        log::debug!("Logical size: {}x{}px", logical_width, logical_height);

        let grid = Grid::new(logical_width, logical_height, settings.grid_spacing);

        log::debug!("Grid size: {}x{}", grid.columns, grid.rows);
        log::debug!("Line count: {}", grid.line_count);

        let basepoint_buffer = Buffer::from_f32(
            context,
            &grid.basepoints,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let line_vertices = Buffer::from_f32(
            context,
            bytemuck::cast_slice(&LINE_VERTICES),
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;
        let plane_vertices = Buffer::from_f32(
            context,
            &data::PLANE_VERTICES,
            glow::ARRAY_BUFFER,
            glow::STATIC_DRAW,
        )?;

        // Programs

        let place_lines_pass = render::Program::new_with_transform_feedback(
            context,
            (PLACE_LINES_VERT_SHADER, PLACE_LINES_FRAG_SHADER),
            &render::TransformFeedbackInfo {
                // The order here must match the order in the buffer!
                names: &[
                    "vEndpointVector",
                    "vVelocityVector",
                    "vColor",
                    "vColorVelocity",
                    "vLineWidth",
                ],
                mode: glow::INTERLEAVED_ATTRIBS,
            },
        )?;
        let draw_lines_pass = render::Program::new(context, (LINE_VERT_SHADER, LINE_FRAG_SHADER))?;
        let draw_endpoints_pass =
            render::Program::new(context, (ENDPOINT_VERT_SHADER, ENDPOINT_FRAG_SHADER))?;
        let draw_texture_pass =
            render::Program::new(context, (TEXTURE_VERT_SHADER, TEXTURE_FRAG_SHADER))?;

        // Vertex buffers

        let mut line_state_buffers =
            render::DoubleTransformFeedback::new(context, bytemuck::cast_slice(&grid.line_state))?;
        let mut place_lines_buffers = Vec::with_capacity(2);
        let mut draw_lines_buffers = Vec::with_capacity(2);
        let mut draw_endpoints_buffers = Vec::with_capacity(2);

        for _ in 0..2 {
            let line_state_buffer = &line_state_buffers.current_buffer().buffer;
            let stride = std::mem::size_of::<LineState>() as u32;
            let common_attributes_with_divisor = |divisor| {
                vec![
                    (
                        &basepoint_buffer,
                        VertexBufferLayout {
                            name: "basepoint",
                            size: 2,
                            type_: glow::FLOAT,
                            divisor,
                            ..Default::default()
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iEndpointVector",
                            size: 2,
                            type_: glow::FLOAT,
                            stride,
                            offset: 0,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iVelocityVector",
                            size: 2,
                            type_: glow::FLOAT,
                            stride,
                            offset: 2 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iColor",
                            size: 4,
                            type_: glow::FLOAT,
                            stride,
                            offset: 4 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iColorVelocity",
                            size: 3,
                            type_: glow::FLOAT,
                            stride,
                            offset: 8 * 4,
                            divisor,
                        },
                    ),
                    (
                        line_state_buffer,
                        VertexBufferLayout {
                            name: "iLineWidth",
                            size: 1,
                            type_: glow::FLOAT,
                            stride,
                            offset: 11 * 4,
                            divisor,
                        },
                    ),
                ]
            };

            place_lines_buffers.push(VertexArrayObject::new(
                context,
                &place_lines_pass,
                &common_attributes_with_divisor(0),
                None,
            )?);

            let mut line_attributes = common_attributes_with_divisor(1);
            line_attributes.push((
                &line_vertices,
                VertexBufferLayout {
                    name: "lineVertex",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            ));
            draw_lines_buffers.push(VertexArrayObject::new(
                context,
                &draw_lines_pass,
                &line_attributes,
                None,
            )?);

            let mut endpoint_attributes = common_attributes_with_divisor(1);
            endpoint_attributes.push((
                &plane_vertices,
                VertexBufferLayout {
                    name: "vertex",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            ));
            draw_endpoints_buffers.push(VertexArrayObject::new(
                context,
                &draw_endpoints_pass,
                &endpoint_attributes,
                None,
            )?);

            line_state_buffers.swap();
        }

        let draw_texture_buffer = VertexArrayObject::new(
            context,
            &draw_texture_pass,
            &[(
                &plane_vertices,
                VertexBufferLayout {
                    name: "position",
                    size: 2,
                    type_: glow::FLOAT,
                    ..Default::default()
                },
            )],
            None,
        )?;

        // Uniforms

        let line_uniforms = UniformBlock::new(
            context,
            LineUniforms::new(
                logical_width as f32,
                logical_height as f32,
                &grid.scaling_ratio,
                settings,
            ),
            0,
            glow::DYNAMIC_DRAW,
        )?;

        place_lines_pass.set_uniforms(&[
            &Uniform {
                name: "velocityTexture",
                value: UniformValue::Texture2D(0),
            },
            &Uniform {
                name: "colorTexture",
                value: UniformValue::Texture2D(1),
            },
        ]);

        place_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_lines_pass.set_uniform_block("LineUniforms", line_uniforms.index);
        draw_endpoints_pass.set_uniform_block("LineUniforms", line_uniforms.index);

        Ok(Self {
            context: Rc::clone(context),
            settings: Rc::clone(settings),

            grid,
            logical_width,
            logical_height,
            physical_width,
            physical_height,

            basepoint_buffer,
            line_state_buffers,
            line_vertices,
            plane_vertices,

            place_lines_buffers,
            draw_lines_buffers,
            draw_endpoints_buffers,
            draw_texture_buffer,

            line_uniforms,
            color_texture: None,

            place_lines_pass,
            draw_lines_pass,
            draw_endpoints_pass,
            draw_texture_pass,
        })
    }

    pub fn update(&mut self, new_settings: &Rc<Settings>) {
        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.update(
                    self.logical_width as f32,
                    self.logical_height as f32,
                    new_settings,
                );
            })
            .buffer_data();
    }

    pub fn is_srgb(&self) -> bool {
        self.line_uniforms.data.color_mode == 2
    }

    pub fn set_color_texture(&mut self, encoded_bytes: &[u8]) -> Result<(), render::Problem> {
        let mut img = image::load_from_memory(encoded_bytes).unwrap();
        if u32::max(img.width(), img.height()) > 640 {
            img = img.resize(640, 400, image::imageops::FilterType::Nearest);
        }

        let color_texture = render::Framebuffer::new(
            &self.context,
            img.width(),
            img.height(),
            render::TextureOptions {
                mag_filter: glow::LINEAR,
                min_filter: glow::LINEAR,
                format: glow::RGB8,
                wrap_s: glow::MIRRORED_REPEAT,
                wrap_t: glow::MIRRORED_REPEAT,
            },
        )?;
        color_texture.with_data(Some(&img.to_rgb8()))?;

        self.color_texture = Some(color_texture);
        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.color_mode = 1;
            })
            .buffer_data();

        Ok(())
    }

    pub fn resize(
        &mut self,
        logical_width: u32,
        logical_height: u32,
        physical_width: u32,
        physical_height: u32,
    ) -> Result<(), render::Problem> {
        let (logical_width, logical_height) = clamp_logical_size(logical_width, logical_height);
        self.physical_width = physical_width;
        self.physical_height = physical_height;
        self.logical_width = logical_width;
        self.logical_height = logical_height;

        let grid = Grid::new(logical_width, logical_height, self.settings.grid_spacing);
        self.basepoint_buffer
            .overwrite(bytemuck::cast_slice(&grid.basepoints));
        self.line_state_buffers
            .overwrite_buffer(bytemuck::cast_slice(&grid.line_state))?;
        self.grid = grid;

        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.update(logical_width as f32, logical_height as f32, &self.settings);
            })
            .buffer_data();

        Ok(())
    }

    pub fn place_lines(
        &mut self,
        velocity_texture: &Framebuffer,
        elapsed_time: f32,
        timestep: f32,
    ) {
        self.line_uniforms
            .update(|line_uniforms| {
                line_uniforms.tick(elapsed_time).set_timestep(timestep);
            })
            .buffer_data();

        unsafe {
            self.place_lines_pass.use_program();
            self.place_lines_buffers[self.line_state_buffers.active_buffer].bind();
            self.line_uniforms.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context
                .bind_texture(glow::TEXTURE_2D, Some(velocity_texture.texture));

            if let Some(ref color_texture) = &self.color_texture {
                self.context.active_texture(glow::TEXTURE1);
                self.context
                    .bind_texture(glow::TEXTURE_2D, Some(color_texture.texture));
            }

            self.line_state_buffers.draw_to(|| {
                self.context
                    .draw_arrays(glow::POINTS, 0, self.grid.line_count as i32);
            });
        }
    }

    pub fn draw_lines(&self) {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.context.enable(glow::BLEND);
            self.context.blend_func(glow::SRC_ALPHA, glow::ONE);

            self.draw_lines_pass.use_program();
            self.draw_lines_buffers[self.line_state_buffers.active_buffer].bind();
            self.line_uniforms.bind();

            self.context
                .draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.grid.line_count as i32);

            self.context.disable(glow::BLEND);
        }
    }

    pub fn draw_endpoints(&self) {
        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.context.enable(glow::BLEND);
            self.context.blend_func(glow::SRC_ALPHA, glow::ONE);

            self.draw_endpoints_pass.use_program();
            self.draw_endpoints_buffers[self.line_state_buffers.active_buffer].bind();
            self.line_uniforms.bind();

            self.context
                .draw_arrays_instanced(glow::TRIANGLES, 0, 6, self.grid.line_count as i32);

            self.context.disable(glow::BLEND);
        }
    }

    pub fn draw_texture(&self, texture: &Framebuffer) {
        if self.color_texture.is_none() {
            return;
        }

        unsafe {
            self.context.viewport(
                0,
                0,
                self.physical_width as i32,
                self.physical_height as i32,
            );

            self.draw_texture_pass.use_program();
            self.draw_texture_buffer.bind();

            self.context.active_texture(glow::TEXTURE0);
            self.context.bind_texture(
                glow::TEXTURE_2D,
                Some(self.color_texture.as_ref().unwrap().texture),
            );

            self.context.draw_arrays(glow::TRIANGLES, 0, 6);
        }
    }

    pub fn scaling_ratio(&self) -> ScalingRatio {
        self.grid.scaling_ratio
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LineState {
    endpoint: mint::Vector2<f32>,
    velocity: mint::Vector2<f32>,
    color: mint::Vector4<f32>,
    color_velocity: mint::Vector3<f32>,
    width: f32,
}

unsafe impl Zeroable for LineState {}
unsafe impl Pod for LineState {}

#[derive(AsStd140)]
struct LineUniforms {
    aspect: f32,
    zoom: f32,
    line_width: f32,
    line_length: f32,
    line_begin_offset: f32,
    line_variance: f32,
    line_noise_scale: mint::Vector2<f32>,
    line_noise_offset_1: f32,
    line_noise_offset_2: f32,
    line_noise_blend_factor: f32,
    pub color_mode: u32,
    delta_time: f32,
}

impl LineUniforms {
    fn new(width: f32, height: f32, scaling_ratio: &ScalingRatio, settings: &Rc<Settings>) -> Self {
        let line_scale_factor = get_line_scale_factor(width, height);
        Self {
            aspect: width / height,
            zoom: settings.view_scale,
            line_width: settings.view_scale * settings.line_width * line_scale_factor,
            line_length: settings.view_scale * settings.line_length * line_scale_factor,
            line_begin_offset: settings.line_begin_offset,
            line_variance: settings.line_variance,
            line_noise_scale: [64.0 * scaling_ratio.x(), 64.0 * scaling_ratio.y()].into(),
            line_noise_offset_1: 0.0,
            line_noise_offset_2: 0.0,
            line_noise_blend_factor: 0.0,
            color_mode: Self::color_scheme_to_mode(&settings.color_scheme),
            delta_time: 0.0,
        }
    }

    fn update(&mut self, width: f32, height: f32, settings: &Rc<Settings>) -> &mut Self {
        let line_scale_factor = get_line_scale_factor(width, height);
        self.aspect = width / height;
        self.zoom = settings.view_scale;
        self.line_width = settings.view_scale * settings.line_width * line_scale_factor;
        self.line_length = settings.view_scale * settings.line_length * line_scale_factor;
        self.line_begin_offset = settings.line_begin_offset;
        self.line_variance = settings.line_variance;
        self.color_mode = Self::color_scheme_to_mode(&settings.color_scheme);
        self
    }

    fn color_scheme_to_mode(color_scheme: &settings::ColorScheme) -> u32 {
        match color_scheme {
            settings::ColorScheme::Peacock => 0,
            _ => 1,
        }
    }

    fn set_timestep(&mut self, timestep: f32) -> &mut Self {
        self.delta_time = timestep;
        self
    }

    fn tick(&mut self, elapsed_time: f32) -> &mut Self {
        const BLEND_THRESHOLD: f32 = 4.0;
        const BASE_OFFSET: f32 = 0.0015;

        let perturb = 1.0 + 0.2 * (0.010 * elapsed_time * std::f32::consts::TAU).sin();
        let offset = BASE_OFFSET * perturb;
        self.line_noise_offset_1 += offset;

        if self.line_noise_offset_1 > BLEND_THRESHOLD {
            self.line_noise_offset_2 += offset;
            self.line_noise_blend_factor += BASE_OFFSET;
        }

        if self.line_noise_blend_factor > 1.0 {
            self.line_noise_offset_1 = self.line_noise_offset_2;
            self.line_noise_offset_2 = 0.0;
            self.line_noise_blend_factor = 0.0;
        }

        self
    }
}

fn get_line_scale_factor(width: f32, height: f32) -> f32 {
    let aspect_ratio = width / height;
    let p = 1.0 / aspect_ratio;
    1.0 / ((1.0 - p) * width + p * height).min(2000.0)
}

fn clamp_logical_size(width: u32, height: u32) -> (u32, u32) {
    let width = width as f32;
    let height = height as f32;

    // TODO: Should we also clamp the upper bound?
    let minimum_dimension = 800.0;
    let scale = f32::max(minimum_dimension / width, minimum_dimension / height).max(1.0);
    (
        (width * scale).floor() as u32,
        (height * scale).floor() as u32,
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalingRatio {
    x: f32,
    y: f32,
}

impl ScalingRatio {
    fn new(columns: u32, rows: u32) -> Self {
        let x = (columns as f32 / 171.0).max(1.0);
        let y = (rows as f32 / 171.0).max(1.0);
        Self { x, y }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn rounded_x(&self) -> u32 {
        self.x.round() as u32
    }

    pub fn rounded_y(&self) -> u32 {
        self.y.round() as u32
    }
}

pub struct Grid {
    columns: u32,
    rows: u32,
    line_count: u32,
    scaling_ratio: ScalingRatio,
    basepoints: Vec<f32>,
    line_state: Vec<LineState>,
}

impl Grid {
    fn new(width: u32, height: u32, grid_spacing: u32) -> Self {
        let height = height as f32;
        let width = width as f32;
        let grid_spacing = grid_spacing as f32;

        let columns = f32::floor(width / grid_spacing);
        let rows = f32::floor((height / width) * columns);
        let grid_spacing_x: f32 = 1.0 / columns;
        let grid_spacing_y: f32 = 1.0 / rows;

        let columns = columns as u32 + 1;
        let rows = rows as u32 + 1;
        let line_count = rows * columns;
        let scaling_ratio = ScalingRatio::new(columns, rows);

        let mut basepoints = Vec::with_capacity(2 * line_count as usize);
        let mut line_state =
            Vec::with_capacity(std::mem::size_of::<LineState>() / 4 * line_count as usize);

        for v in 0..rows {
            for u in 0..columns {
                basepoints.push(u as f32 * grid_spacing_x);
                basepoints.push(v as f32 * grid_spacing_y);

                line_state.push(LineState {
                    endpoint: [0.0, 0.0].into(),
                    velocity: [0.0, 0.0].into(),
                    color: [0.0, 0.0, 0.0, 0.0].into(),
                    color_velocity: [0.0, 0.0, 0.0].into(),
                    width: 0.0,
                });
            }
        }

        Self {
            columns,
            rows,
            scaling_ratio,
            line_count,
            basepoints,
            line_state,
        }
    }
}

static LINE_VERT_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/line.vert"));
static LINE_FRAG_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/line.frag"));
static ENDPOINT_VERT_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/endpoint.vert"));
static ENDPOINT_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/endpoint.frag"));
static TEXTURE_VERT_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/texture.vert"));
static TEXTURE_FRAG_SHADER: &str = include_str!(concat!(env!("OUT_DIR"), "/shaders/texture.frag"));
static PLACE_LINES_VERT_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/place_lines.vert"));
static PLACE_LINES_FRAG_SHADER: &str =
    include_str!(concat!(env!("OUT_DIR"), "/shaders/place_lines.frag"));

#[rustfmt::skip]
const LINE_VERTICES: [f32; 12] = [
    -0.5, 0.0,
    -0.5, 1.0,
     0.5, 1.0,
    -0.5, 0.0,
     0.5, 1.0,
     0.5, 0.0,
];

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Copy, Clone, PartialEq, Debug)]
    struct LogicalSize {
        pub width: u32,
        pub height: u32,
    }

    impl LogicalSize {
        pub fn new(width: u32, height: u32) -> Self {
            Self { width, height }
        }
    }

    fn create_test_grid(logical_size: LogicalSize, grid_spacing: u32) -> (u32, u32) {
        let Grid { columns, rows, .. } =
            Grid::new(logical_size.width, logical_size.height, grid_spacing);
        (columns, rows)
    }

    #[test]
    fn is_sane_grid_for_iphone_xr() {
        let logical_size = LogicalSize::new(414, 896);
        assert_eq!(create_test_grid(logical_size, 15), (28, 59));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (800, 1731)
        );
    }

    #[test]
    fn is_sane_grid_for_iphone_12_pro() {
        let logical_size = LogicalSize::new(390, 844);
        assert_eq!(create_test_grid(logical_size, 15), (27, 57));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (800, 1731)
        );
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_13_with_1280_800_scaling() {
        let logical_size = LogicalSize::new(1280, 800);
        assert_eq!(create_test_grid(logical_size, 15), (86, 54));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (1280, 800)
        );
    }

    #[test]
    fn is_sane_grid_for_macbook_pro_15_with_1440_900_scaling() {
        let logical_size = LogicalSize::new(1440, 900);
        assert_eq!(create_test_grid(logical_size, 15), (97, 61));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (1440, 900)
        );
    }

    #[test]
    fn is_sane_grid_for_ultrawide_4k() {
        let logical_size = LogicalSize::new(3840, 1600);
        assert_eq!(create_test_grid(logical_size, 15), (257, 107));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (3840, 1600)
        );
    }

    #[test]
    fn is_sane_grid_for_triple_2560_1440() {
        let logical_size = LogicalSize::new(2560 * 3, 1440);
        assert_eq!(create_test_grid(logical_size, 15), (513, 97));
        assert_eq!(
            clamp_logical_size(logical_size.width, logical_size.height),
            (logical_size.width, logical_size.height)
        );
    }
}
