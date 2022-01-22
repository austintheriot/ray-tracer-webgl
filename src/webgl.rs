use std::sync::MutexGuard;

use crate::{dom, state::State};
use futures::try_join;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Request, Response, WebGl2RenderingContext, WebGlFramebuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlUniformLocation,
};

pub const SIMPLE_QUAD_VERTICES: [f32; 12] = [
    -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0,
];

pub fn compile_shader(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

pub fn link_program(
    gl: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}

pub async fn setup_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    let (fragment_source, vertex_source) =
        try_join!(fetch_shader("./shader.frag"), fetch_shader("./shader.vert"))?;

    let vertex_shader = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, &vertex_source)?;
    let fragment_shader = compile_shader(
        gl,
        WebGl2RenderingContext::FRAGMENT_SHADER,
        &fragment_source,
    )?;
    let program = link_program(&gl, &vertex_shader, &fragment_shader)?;
    gl.use_program(Some(&program));

    Ok(program)
}

pub fn create_texture(gl: &WebGl2RenderingContext, state: &MutexGuard<State>) -> WebGlTexture {
    let texture = gl.create_texture();
    gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, texture.as_ref());

    // Set the parameters so we don't need mips, we're not filtering, and we don't repeat
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_WRAP_S,
        WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_WRAP_T,
        WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_MIN_FILTER,
        WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_MAG_FILTER,
        WebGl2RenderingContext::LINEAR as i32,
    );

    // load empty texture into gpu -- this will get rendered into later
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        WebGl2RenderingContext::TEXTURE_2D,
        0,
        WebGl2RenderingContext::RGBA as i32,
        state.width as i32,
        state.height as i32,
        0,
        WebGl2RenderingContext::RGBA,
        WebGl2RenderingContext::UNSIGNED_BYTE,
        None,
    )
    .unwrap();
    drop(state);

    texture.unwrap()
}

pub fn setup_vertex_buffer(
    gl: &WebGl2RenderingContext,
    program: &WebGlProgram,
) -> Result<(), JsValue> {
    let vertex_attribute_position = gl.get_attrib_location(program, "a_position") as u32;
    let buffer = gl.create_buffer().ok_or("failed to create buffer")?;
    gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));
    // requires `unsafe` since we're creating a raw view into wasm memory,
    // but this array is static, so it shouldn't cause any issues
    let vertex_array = unsafe { js_sys::Float32Array::view(&SIMPLE_QUAD_VERTICES) };
    gl.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &vertex_array,
        WebGl2RenderingContext::STATIC_DRAW,
    );
    gl.enable_vertex_attrib_array(vertex_attribute_position);
    gl.vertex_attrib_pointer_with_i32(
        vertex_attribute_position,
        2,
        WebGl2RenderingContext::FLOAT,
        false,
        0,
        0,
    );

    Ok(())
}

pub fn create_framebuffer(gl: &WebGl2RenderingContext, texture: &WebGlTexture) -> WebGlFramebuffer {
    let framebuffer_object = gl.create_framebuffer();
    gl.bind_framebuffer(
        WebGl2RenderingContext::FRAMEBUFFER,
        framebuffer_object.as_ref(),
    );
    gl.framebuffer_texture_2d(
        WebGl2RenderingContext::FRAMEBUFFER,
        WebGl2RenderingContext::COLOR_ATTACHMENT0,
        WebGl2RenderingContext::TEXTURE_2D,
        Some(&texture),
        0,
    );
    framebuffer_object.unwrap()
}

pub fn draw(gl: &WebGl2RenderingContext, state: &MutexGuard<State>) {
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.viewport(0, 0, state.width as i32, state.height as i32);
    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    gl.draw_arrays(
        WebGl2RenderingContext::TRIANGLES,
        0,
        (SIMPLE_QUAD_VERTICES.len() / 2) as i32,
    );
}

pub fn render(
    gl: &WebGl2RenderingContext,
    state: &MutexGuard<State>,
    textures: &[WebGlTexture; 2],
    framebuffer_objects: &[WebGlFramebuffer; 2],
) {
    // use texture previously rendered to
    gl.bind_texture(
        WebGl2RenderingContext::TEXTURE_2D,
        Some(&textures[((state.even_odd_count + 1) % 2) as usize]),
    );

    // draw to canvas
    gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);
    draw(&gl, &state);

    // only need to draw to framebuffer when doing averages of previous frames
    if state.should_average {
        // RENDER (TO FRAMEBUFFER)
        gl.bind_framebuffer(
            WebGl2RenderingContext::FRAMEBUFFER,
            Some(&framebuffer_objects[(state.even_odd_count % 2) as usize]),
        );
        draw(&gl, &state);
    }
}

pub async fn fetch_shader(url: &str) -> Result<String, JsValue> {
    let request = Request::new_with_str(url)?;
    let resp_value = JsFuture::from(dom::window().fetch_with_request(&request)).await?;

    // `resp_value` is a `Response` object.
    assert!(resp_value.is_instance_of::<Response>());
    let resp: Response = resp_value.dyn_into()?;

    // Convert this other `Promise` into a rust `Future`.
    let text = JsFuture::from(resp.text()?)
        .await?
        .as_string()
        .ok_or("Couldn't convert shader source into String")?;

    Ok(text)
}

// iterates through list of hittable geometry and sets uniforms at initialization time
pub fn set_geometry(
    state: &MutexGuard<State>,
    gl: &WebGl2RenderingContext,
    program: &WebGlProgram,
) {
    for (i, sphere) in state.sphere_list.iter().enumerate() {
        let sphere_center_location =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].center", i));
        gl.uniform3fv_with_f32_array(sphere_center_location.as_ref(), &sphere.center.to_array());

        let sphere_radius_location =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].radius", i));
        gl.uniform1f(sphere_radius_location.as_ref(), sphere.radius);

        let sphere_material_type_location =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].material.type", i));
        gl.uniform1i(
            sphere_material_type_location.as_ref(),
            sphere.material.material_type.value(),
        );

        let sphere_material_albedo_location =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].material.albedo", i));
        gl.uniform3fv_with_f32_array(
            sphere_material_albedo_location.as_ref(),
            &sphere.material.albedo.to_array(),
        );

        let sphere_material_fuzz_location =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].material.fuzz", i));
        gl.uniform1f(sphere_material_fuzz_location.as_ref(), sphere.material.fuzz);

        let sphere_material_refraction_index_location = gl.get_uniform_location(
            &program,
            &format!("u_sphere_list[{}].material.refraction_index", i),
        );
        gl.uniform1f(
            sphere_material_refraction_index_location.as_ref(),
            sphere.material.refraction_index,
        );

        let sphere_is_active =
            gl.get_uniform_location(&program, &format!("u_sphere_list[{}].is_active", i));
        gl.uniform1i(sphere_is_active.as_ref(), true as i32);
    }
}

/// Kind of hacky, but allows setting up uniform names and how to update them once.
/// The location of each uniform is saved on creation, and then each uniform is updated
/// automatically on every render
pub fn setup_uniforms(gl: &WebGl2RenderingContext, program: &WebGlProgram) -> Uniforms {
    Uniforms::create(
        gl,
        program,
        vec![
            Uniform {
                name: "u_texture",
                updater: Box::new(
                    |_: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1i(location.as_ref(), 0);
                    },
                ),
            },
            Uniform {
                name: "u_width",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.width as f32);
                    },
                ),
            },
            Uniform {
                name: "u_height",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.height as f32);
                    },
                ),
            },
            Uniform {
                name: "u_time",
                updater: Box::new(
                    |_: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     now: f64| {
                        gl.uniform1f(location.as_ref(), now as f32);
                    },
                ),
            },
            Uniform {
                name: "u_samples_per_pixel",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        // increase sample rate when paused (such as on first render and when resizing)
                        // it's ok to do some heavy lifting here, since it's not being continually rendered at this output
                        let samples_per_pixel = if state.is_paused {
                            state.samples_per_pixel.max(25)
                        } else {
                            state.samples_per_pixel
                        };
                        gl.uniform1i(location.as_ref(), samples_per_pixel as i32);
                    },
                ),
            },
            Uniform {
                name: "u_aspect_ratio",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.aspect_ratio as f32);
                    },
                ),
            },
            Uniform {
                name: "u_viewport_height",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.viewport_height as f32);
                    },
                ),
            },
            Uniform {
                name: "u_viewport_width",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.viewport_width as f32);
                    },
                ),
            },
            Uniform {
                name: "u_focal_length",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.focal_length as f32);
                    },
                ),
            },
            Uniform {
                name: "u_camera_origin",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(
                            location.as_ref(),
                            &state.camera_origin.to_array(),
                        );
                    },
                ),
            },
            Uniform {
                name: "u_horizontal",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(
                            location.as_ref(),
                            &state.horizontal.to_array(),
                        );
                    },
                ),
            },
            Uniform {
                name: "u_vertical",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(location.as_ref(), &state.vertical.to_array());
                    },
                ),
            },
            Uniform {
                name: "u_lower_left_corner",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(
                            location.as_ref(),
                            &state.lower_left_corner.to_array(),
                        );
                    },
                ),
            },
            Uniform {
                name: "u_max_depth",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1i(location.as_ref(), state.max_depth as i32);
                    },
                ),
            },
            Uniform {
                name: "u_render_count",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1i(location.as_ref(), state.render_count as i32);
                    },
                ),
            },
            Uniform {
                name: "u_should_average",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1i(location.as_ref(), state.should_average as i32);
                    },
                ),
            },
            Uniform {
                name: "u_last_frame_weight",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.last_frame_weight as f32);
                    },
                ),
            },
            Uniform {
                name: "u_lens_radius",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform1f(location.as_ref(), state.lens_radius as f32);
                    },
                ),
            },
            Uniform {
                name: "u_u",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(location.as_ref(), &state.u.to_array());
                    },
                ),
            },
            Uniform {
                name: "u_v",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(location.as_ref(), &state.v.to_array());
                    },
                ),
            },
            Uniform {
                name: "u_w",
                updater: Box::new(
                    |state: &MutexGuard<State>,
                     location: &Option<WebGlUniformLocation>,
                     gl: &WebGl2RenderingContext,
                     _: f64| {
                        gl.uniform3fv_with_f32_array(location.as_ref(), &state.w.to_array());
                    },
                ),
            },
        ],
    )
}

pub struct Uniform {
    pub name: &'static str,
    pub updater: Box<
        dyn Fn(&MutexGuard<State>, &Option<WebGlUniformLocation>, &WebGl2RenderingContext, f64),
    >,
}

pub struct UniformWithLocation {
    pub name: &'static str,
    location: Option<WebGlUniformLocation>,
    pub updater: Box<
        dyn Fn(&MutexGuard<State>, &Option<WebGlUniformLocation>, &WebGl2RenderingContext, f64),
    >,
}

pub struct Uniforms {
    pub list: Vec<UniformWithLocation>,
}

impl Uniforms {
    // once all uniforms are passed in, their WebGlUniformLocations are looked up
    // and saved for passing in later when updating
    pub fn create(
        gl: &WebGl2RenderingContext,
        program: &WebGlProgram,
        uniform_list: Vec<Uniform>,
    ) -> Self {
        Uniforms {
            list: uniform_list
                .into_iter()
                .map(|uniform| UniformWithLocation {
                    location: gl.get_uniform_location(program, uniform.name),
                    name: uniform.name,
                    updater: uniform.updater,
                })
                .collect(),
        }
    }

    // set uniforms with current state
    pub fn run_setters(&self, state: &MutexGuard<State>, gl: &WebGl2RenderingContext, now: f64) {
        for uniform in self.list.iter() {
            (uniform.updater)(state, &uniform.location, gl, now);
        }
    }
}
