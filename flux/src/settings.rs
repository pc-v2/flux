use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub mode: Mode,

    pub fluid_size: u32,
    pub fluid_frame_rate: f32,
    pub fluid_timestep: f32,
    pub viscosity: f32,
    pub velocity_dissipation: f32,
    pub clear_pressure: ClearPressure,
    pub diffusion_iterations: u32,
    pub pressure_iterations: u32,

    pub color_scheme: ColorScheme,

    pub line_length: f32,
    pub line_width: f32,
    pub line_begin_offset: f32,
    pub line_variance: f32,
    pub grid_spacing: u32,
    pub view_scale: f32,

    pub noise_channels: Vec<Noise>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Mode {
    Normal,
    DebugNoise,
    DebugFluid,
    DebugPressure,
    DebugDivergence,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ClearPressure {
    KeepPressure,
    ClearPressure(f32),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ColorScheme {
    Plasma,
    Peacock,
    Poolside,
    Freedom,
    FromImage(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Noise {
    pub scale: f32,
    pub multiplier: f32,
    pub offset_increment: f32,
}

// pub fn color_wheel_from_scheme(color_scheme: &ColorScheme) -> [f32; 24] {
//     match color_scheme {
//         ColorScheme::Plasma => COLOR_SCHEME_PLASMA,
//         ColorScheme::Poolside => COLOR_SCHEME_POOLSIDE,
//         ColorScheme::Freedom => COLOR_SCHEME_FREEDOM,
//         _ => [0.0; 24],
//     }
// }

// #[rustfmt::skip]
// pub static COLOR_SCHEME_PLASMA: [f32; 24] = [
//     60.219  / 255.0, 37.2487 / 255.0, 66.4301 / 255.0, 1.0,
//     170.962 / 255.0, 54.4873 / 255.0, 50.9661 / 255.0, 1.0,
//     230.299 / 255.0, 39.2759 / 255.0, 5.54531 / 255.0, 1.0,
//     242.924 / 255.0, 94.3563 / 255.0, 22.4186 / 255.0, 1.0,
//     242.435 / 255.0, 156.752 / 255.0, 58.9794 / 255.0, 1.0,
//     135.291 / 255.0, 152.793 / 255.0, 182.473 / 255.0, 1.0,
// ];
// #[rustfmt::skip]
// pub static COLOR_SCHEME_POOLSIDE: [f32; 24] = [
//     76.0 / 255.0, 156.0 / 255.0, 228.0 / 255.0, 1.0,
//     140.0 / 255.0, 204.0 / 255.0, 244.0 / 255.0, 1.0,
//     108.0 / 255.0, 180.0 / 255.0, 236.0 / 255.0, 1.0,
//     188.0 / 255.0, 228.0 / 255.0, 244.0 / 255.0, 1.0,
//     124.0 / 255.0, 220.0 / 255.0, 236.0 / 255.0, 1.0,
//     156.0 / 255.0, 208.0 / 255.0, 236.0 / 255.0, 1.0,
// ];
// #[rustfmt::skip]
// pub static COLOR_SCHEME_FREEDOM: [f32; 24] = [
//     0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
//     0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
//     0.0 / 255.0,   87.0 / 255.0,  183.0 / 255.0, 1.0, // blue
//     1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
//     1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
//     1.0,           215.0 / 255.0, 0.0,           1.0, // yellow
// ];
