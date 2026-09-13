use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{WebGlProgram, WebGlRenderingContext, WebGlShader, WebGlUniformLocation};

// Vertex Shader with a Uniform Matrix for 3D rotation and perspective projection
const VERTEX_SHADER_SRC: &str = r#"
    attribute vec3 position;
    uniform mat4 uMatrix;
    void main(void) {
        gl_Position = uMatrix * vec4(position, 1.0);
    }
"#;

// Fragment Shader rendering a transparent sky blue color
const FRAGMENT_SHADER_SRC: &str = r#"
    precision mediump float;
    void main(void) {
        // Sky Blue: R=0.53, G=0.81, B=0.98. Alpha=0.6 for transparency
        gl_FragColor = vec4(0.53, 0.81, 0.98, 0.6);
    }
"#;

#[wasm_bindgen]
pub fn start(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no global `window` exists")?;
    let document = window.document().ok_or("should have a document on window")?;
    let canvas = document.get_element_by_id(canvas_id)
        .ok_or_else(|| format!("unable to find canvas element '{}'", canvas_id))?;
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into::<web_sys::HtmlCanvasElement>()?;

    let gl = canvas
        .get_context("webgl")?
        .ok_or("WebGL not supported")?
        .dyn_into::<WebGlRenderingContext>()?;

    // Compile Shaders and Link Program
    let vert_shader = compile_shader(&gl, WebGlRenderingContext::VERTEX_SHADER, VERTEX_SHADER_SRC)?;
    let frag_shader = compile_shader(&gl, WebGlRenderingContext::FRAGMENT_SHADER, FRAGMENT_SHADER_SRC)?;
    let program = link_program(&gl, &vert_shader, &frag_shader)?;
    gl.use_program(Some(&program));

    // Define 3D Cube Vertices (Only positions are needed now)
    let vertices: [f32; 24] = [
        -0.5, -0.5,  0.5,   0.5, -0.5,  0.5,   0.5,  0.5,  0.5,  -0.5,  0.5,  0.5, // Front
        -0.5, -0.5, -0.5,   0.5, -0.5, -0.5,   0.5,  0.5, -0.5,  -0.5,  0.5, -0.5, // Back
    ];

    let indices: [u16; 36] = [
        0, 1, 2,  0, 2, 3, // Front
        5, 4, 7,  5, 7, 6, // Back
        3, 2, 6,  3, 6, 7, // Top
        4, 5, 1,  4, 1, 0, // Bottom
        1, 5, 6,  1, 6, 2, // Right
        4, 0, 3,  4, 3, 7, // Left
    ];

    // Create and Upload Buffers
    let vertex_buffer = gl.create_buffer().ok_or("failed to create buffer")?;
    gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&vertex_buffer));
    unsafe {
        let view = js_sys::Float32Array::view(&vertices);
        gl.buffer_data_with_array_buffer_view(WebGlRenderingContext::ARRAY_BUFFER, &view, WebGlRenderingContext::STATIC_DRAW);
    }

    let index_buffer = gl.create_buffer().ok_or("failed to create index buffer")?;
    gl.bind_buffer(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));
    unsafe {
        let view = js_sys::Uint16Array::view(&indices);
        gl.buffer_data_with_array_buffer_view(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, &view, WebGlRenderingContext::STATIC_DRAW);
    }

    let position_loc = gl.get_attrib_location(&program, "position") as u32;
    gl.vertex_attrib_pointer_with_i32(position_loc, 3, WebGlRenderingContext::FLOAT, false, 0, 0);
    gl.enable_vertex_attrib_array(position_loc);

    let matrix_loc = gl.get_uniform_location(&program, "uMatrix").ok_or("Matrix uniform not found")?;

    // Enable Alpha Blending for Transparency
    gl.enable(WebGlRenderingContext::BLEND);
    gl.blend_func(WebGlRenderingContext::SRC_ALPHA, WebGlRenderingContext::ONE_MINUS_SRC_ALPHA);
    
    // For clean transparency visualizations, we bypass depth-mask blocking
    gl.enable(WebGlRenderingContext::DEPTH_TEST);

    // Track state: rotation angles and mouse interaction
    let rotation = Rc::new(RefCell::new((0.5f32, 0.5f32))); // X and Y rotation angles
    let is_dragging = Rc::new(RefCell::new(false));

    // Mouse Down Event Listener
    {
        let is_dragging = is_dragging.clone();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            *is_dragging.borrow_mut() = true;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouse Up Event Listener
    {
        let is_dragging = is_dragging.clone();
        let closure = Closure::wrap(Box::new(move |_event: web_sys::MouseEvent| {
            *is_dragging.borrow_mut() = false;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Mouse Move Event Listener (Updates rotation dynamically when dragging)
    {
        let is_dragging = is_dragging.clone();
        let rotation = rotation.clone();
        let closure = Closure::wrap(Box::new(move |event: web_sys::MouseEvent| {
            if *is_dragging.borrow() {
                let mut rot = rotation.borrow_mut();
                rot.0 += event.movement_y() as f32 * 0.01; // Pitch
                rot.1 += event.movement_x() as f32 * 0.01; // Yaw
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Animation Render Loop Configuration
       // Animation Render Loop Configuration
    // FIX: Explicitly type the RefCell Option so the compiler knows it contains our closure
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let (rot_x, rot_y) = *rotation.borrow();
        
        // Construct basic 3D Model-View-Perspective matrix multiplication manually
        let matrix = simple_3d_matrix(rot_x, rot_y, 600.0 / 600.0);

        gl.clear_color(0.1, 0.1, 0.1, 1.0);
        gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT | WebGlRenderingContext::DEPTH_BUFFER_BIT);

        gl.uniform_matrix4fv_with_f32_array(Some(&matrix_loc), false, &matrix);
        gl.draw_elements_with_i32(WebGlRenderingContext::TRIANGLES, 36, WebGlRenderingContext::UNSIGNED_SHORT, 0);

        // Request next frame execution
        web_sys::window().unwrap().request_animation_frame(
            f.borrow().as_ref().unwrap().as_ref().unchecked_ref()
        ).unwrap();
    }) as Box<dyn FnMut()>));

    // Start the loop execution
    web_sys::window().unwrap().request_animation_frame(
        g.borrow().as_ref().unwrap().as_ref().unchecked_ref()
    ).unwrap();

    Ok(())
}

// Generates a simple combined perspective, translation, and rotation 4x4 Matrix
fn simple_3d_matrix(rx: f32, ry: f32, aspect: f32) -> [f32; 16] {
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();

    // Rotation around X
    let rx_mat = [
        1.0, 0.0, 0.0, 0.0,
        0.0, cx,  sx,  0.0,
        0.0, -sx, cx,  0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    // Rotation around Y
    let ry_mat = [
        cy,  0.0, -sy, 0.0,
        0.0, 1.0, 0.0, 0.0,
        sy,  0.0, cy,  0.0,
        0.0, 0.0, 0.0, 1.0,
    ];

    // Perspective projection
    let fov = 45.0_f32.to_radians();
    let f = 1.0 / (fov / 2.0).tan();

    let near = 0.1;
    let far = 100.0;

    let projection = [
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) / (near - far), -1.0,
        0.0, 0.0, (2.0 * far * near) / (near - far), 0.0,
    ];

    // Translation: move cube away from camera
    let translation = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, -2.0, 1.0,
    ];

    let rotation = multiply_matrix(&ry_mat, &rx_mat);
    let model_view = multiply_matrix(&translation, &rotation);

    multiply_matrix(&projection, &model_view)
}

fn multiply_matrix(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut result = [0.0; 16];

    for col in 0..4 {
        for row in 0..4 {
            result[col * 4 + row] =
                a[0 * 4 + row] * b[col * 4 + 0]
                + a[1 * 4 + row] * b[col * 4 + 1]
                + a[2 * 4 + row] * b[col * 4 + 2]
                + a[3 * 4 + row] * b[col * 4 + 3];
        }
    }

    result
}
fn compile_shader(gl: &WebGlRenderingContext, shader_type: u32, source: &str) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or_else(|| String::from("Shader creation error"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl.get_shader_parameter(&shader, WebGlRenderingContext::COMPILE_STATUS).as_bool().unwrap_or(false) {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_else(|| String::from("Shader compile failed")))
    }
}

fn link_program(gl: &WebGlRenderingContext, vert_shader: &WebGlShader, frag_shader: &WebGlShader) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or_else(|| String::from("Program creation error"))?;
    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);
    if gl.get_program_parameter(&program, WebGlRenderingContext::LINK_STATUS).as_bool().unwrap_or(false) {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_else(|| String::from("Program linking failed")))
    }
}
