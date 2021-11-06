mod data;
mod drawer;
mod fluid;
mod render;
mod web;

use fluid::Fluid;
use web::ContextOptions;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::WebGl2RenderingContext as GL;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    web::set_panic_hook();

    let window = web::window();
    let document = window.document().unwrap();
    let canvas = document.get_element_by_id("canvas").unwrap();
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;
    let width = canvas.width();
    let height = canvas.height();

    let options = ContextOptions {
        alpha: true,
        preserve_drawing_buffer: false,
        stencil: false,
        premultiplied_alpha: true,
        power_preference: "high-performance",
        depth: true,
        antialias: false,
    }
    .serialize();

    let gl = canvas
        .get_context_with_context_options("webgl2", &options)?
        .unwrap()
        .dyn_into::<GL>()?;
    gl.get_extension("OES_texture_float")?;
    gl.get_extension("OES_texture_float_linear")?;
    gl.get_extension("EXT_color_buffer_float")?;
    gl.get_extension("EXT_float_blend")?;

    gl.disable(GL::BLEND);
    gl.disable(GL::DEPTH_TEST);

    let context = Rc::new(gl);

    // Settings
    let grid_spacing: f32 = 10.0;
    let grid_width: u32 = ((width as f32) / grid_spacing).ceil() as u32;
    let grid_height: u32 = ((height as f32) / grid_spacing).ceil() as u32;

    let delta_t: f32 = 1.0 / 120.0;
    let viscosity: f32 = 1.0; // rho
    let velocity_dissipation: f32 = 0.3;

    // TODO: deal with result
    let fluid = Fluid::new(
        &context,
        grid_width,
        grid_height,
        viscosity,
        velocity_dissipation,
    )
    .unwrap();

    // let drawer = Drawer::new(&context, width, height);

    // TODO: clean this up
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let animate: Box<dyn FnMut(f32)> = Box::new(move |_| {
        // Clear the canvas
        context.clear_color(0.0, 0.0, 0.0, 1.0);
        context.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        context.viewport(0, 0, width as i32, height as i32);

        // Advection
        {
            fluid.advect(delta_t);
        }

        web::request_animation_frame(f.borrow().as_ref().unwrap());
    });

    *g.borrow_mut() = Some(Closure::wrap(animate));
    web::request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}
