#![cfg(target_os = "android")]
#![allow(non_snake_case)]

extern crate android_logger;

use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr::null;

use anyhow::{anyhow, Context, Error};
use khronos_egl::{choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, initialize, make_current, NO_CONTEXT, GL_COLORSPACE, GL_COLORSPACE_SRGB, swap_buffers, query_surface, create_pixmap_surface, choose_config};
use rs_gles3::{GL_ARRAY_BUFFER, GL_COLOR_BUFFER_BIT, GL_COMPILE_STATUS, GL_ELEMENT_ARRAY_BUFFER, GL_FALSE, GL_FLOAT, GL_FRAGMENT_SHADER, GL_INVALID_ENUM, GL_INVALID_FRAMEBUFFER_OPERATION, GL_INVALID_OPERATION, GL_INVALID_VALUE, GL_LINK_STATUS, GL_OUT_OF_MEMORY, GL_STATIC_DRAW, GL_TRIANGLES, GL_TRUE, GL_UNSIGNED_SHORT, GL_VERTEX_SHADER, glAttachShader, glBindBuffer, glBindVertexArray, glBufferData, GLchar, glClear, glCompileShader, glCreateProgram, glCreateShader, glDeleteProgram, glDeleteShader, glDetachShader, glDrawElements, glDrawElementsInstanced, glEnableVertexAttribArray, GLenum, GLfloat, glGenBuffers, glGenVertexArrays, glGetError, glGetProgramiv, glGetShaderiv, glGetUniformLocation, GLint, glLinkProgram, glReadPixels, glShaderSource, GLuint, glUniformMatrix4fv, glUseProgram, glVertexAttribPointer, glCopyTexImage2D, GL_RGB_INTEGER, GL_RGBA_INTEGER, GL_UNSIGNED_BYTE, glFinish, glPixelStorei, GL_UNPACK_ALIGNMENT, glBindFramebuffer, glViewport, glClearColor, GL_RGBA, GL_DEPTH_BUFFER_BIT, glDisable, GL_CULL_FACE, GL_DEPTH, GL_DEPTH_TEST, glDrawArrays, glGetAttribLocation, GL_LINES, GL_PACK_ALIGNMENT, glValidateProgram, GL_POINTS, glGetProgramBinary, GL_VALIDATE_STATUS};
use std::fs::File;
use std::io::Write;
use std::env;

#[no_mangle]
pub extern fn Java_com_mersive_glconvert_MainActivity_init(
    env: JNIEnv,
   _obj: JObject,
) {
    android_logger::init_once(Config::default().with_min_level(Level::Debug));
    info!("Hello, Rust!");
}

fn main() -> Result<(), Error> {
    unsafe {
        let args: Vec<String> = env::args().collect();
        let idx = args.get(1).unwrap().parse::<usize>().unwrap();

        // egl init
        let display = get_display(DEFAULT_DISPLAY).expect("Need a display!");
        let attributes = [
            khronos_egl::NONE
        ];
        let res = initialize(display).expect("Can't initialize");
        println!("EGL version={:?}", res);
        let mut configs: Vec<Config> = Vec::with_capacity(100);
        choose_config(display, &attributes, &mut configs)
            .expect("unable to choose an EGL configuration");
        println!("count={}", configs.len());
        let config = configs.remove(idx);
        let ctx = create_context(display, config, None, &attributes).expect("Need a context!");
        println!("EGL context={:?}", ctx);

        // create surface
        let width = 350;
        let height = 350;
        let mut attributes = vec![
            khronos_egl::WIDTH, width.clone(),
            khronos_egl::HEIGHT, height.clone(),
        ];
        #[cfg(os="linux")]
        {
            attributes.extend_from_slice(&[khronos_egl::TEXTURE_FORMAT, khronos_egl::TEXTURE_RGBA]);
            attributes.extend_from_slice(&[khronos_egl::TEXTURE_TARGET, khronos_egl::TEXTURE_2D]);
        }
        attributes.push(khronos_egl::NONE);
        let surface = create_pbuffer_surface(display, config, &attributes).expect("Couldn't create pbuffer");
        // create_pixmap_surface(display, config);
        make_current(display, Some(surface.clone()), Some(surface.clone()), Some(ctx)).expect("Can't make current");
        let w = query_surface(display, surface, khronos_egl::WIDTH).expect("Can't get width!");
        let h = query_surface(display, surface, khronos_egl::HEIGHT).expect("Can't get HEIGHT!");
        println!("w={} h={}", w, h);

        // https://github.com/AlexCharlton/hello-modern-opengl/blob/master/hello-gl.c
        #[cfg(target_os="android")]
        let ver = "300 es";
        #[cfg(target_os="linux")]
        let ver = "330";

        let vert_shader = format!("\
#version {}\n\
in vec3 vertex;\n\
void main(){{\n\
    gl_Position = vec4(vertex, 1.0);\n\
}}\n\0", ver);

        let frag_shader = format!("\
#version {}\n\
out vec4 color;\n\
void main(){{\n\
    color = vec4(1.0, 1.0, 1.0, 1.0);\n\
}}\n\0", ver);

        let program_id = make_shader(vert_shader.as_str(), frag_shader.as_str()).expect("Couldn't make shader");
        let name = "vertex\0".as_ptr() as *const GLchar;
        let attr_id = glGetAttribLocation(program_id, name);
        println!("Have a program={} attr_id={}", program_id, attr_id);

        let mut vertex_buffer_data: Vec<f32> = vec![
            -0.5, -0.5, 0.,
            0.5, -0.5, 0.,
            0.5, 0.5, 0.,

            0.5, 0.5, 0.,
            -0.5, 0.5, 0.,
            -0.5, -0.5, 0.,
        ];

        // vert buf
        let mut vertex_buffer: GLuint = 0;
        glGenBuffers(1, &mut vertex_buffer);
        if vertex_buffer == 0 { panic!("Invalid vertex buffer!"); }
        glBindBuffer(GL_ARRAY_BUFFER, vertex_buffer);
        check_error().context("Cannot bind buffer!")?;
        glBufferData(GL_ARRAY_BUFFER, (vertex_buffer_data.len() * 4) as i64, vertex_buffer_data.as_ptr() as *const c_void, GL_STATIC_DRAW);
        check_error().context("Cannot set buffer data")?;

        // vertex array
        let mut vertex_array_id: GLuint = 0;
        glGenVertexArrays(1, &mut vertex_array_id);
        if vertex_array_id == 0 { panic!("Invalid vertex array!"); }
        glBindVertexArray(vertex_array_id);
        check_error().context("Cannot bind buffer")?;
        glEnableVertexAttribArray(attr_id as u32);
        glVertexAttribPointer(attr_id as u32, 3, GL_FLOAT, GL_FALSE as u8, 0, null());
        check_error().context("Cannot set vertex attrib pointer")?;

        glViewport(0, 0, width, height);
        glClearColor(0.5, 0.5, 0.5, 1.0);
        glClear(GL_COLOR_BUFFER_BIT);
        check_error().context("Cannot clear!")?;

        glUseProgram(program_id);
        glPixelStorei(GL_PACK_ALIGNMENT, 1);
        glValidateProgram(program_id);
        let mut res: GLint = 0;
        glGetProgramiv(program_id, GL_VALIDATE_STATUS, &mut res as *mut GLint);
        if res == 0 { panic!("Bad program") }
        check_error().expect("Invalid program!");

        // glDrawArrays(GL_POINTS, 0, (vertex_buffer_data.len() / 3) as i32);
        glDrawArrays(GL_TRIANGLES, 0, (vertex_buffer_data.len() / 3) as i32);
        check_error().context("Cannot draw!")?;
        glFinish();
        check_error().context("Cannot finish!")?;

        let mut pixels = vec![0u8; (width * height) as usize * 4];
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        check_error().context("Cannot set pixel store mode!")?;
        glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut c_void);
        check_error().context("Cannot get pixels!")?;

        // Save
        let mut file = File::create(format!("pic{}.raw", idx))?;
        file.write_all(&pixels[..])?;
    }

    Ok(())
}

pub unsafe fn make_shader(vertex_source: &str, fragment_source: &str) -> Result<u32, Error> {
    let mut program_ok: GLint = 0;
    let vertex_shader = make_shader_object(GL_VERTEX_SHADER, vertex_source).context("Cannot compile vertex shader")?;
    let fragment_shader = make_shader_object(GL_FRAGMENT_SHADER, fragment_source).context("Cannot compile fragment shader")?;
    if vertex_shader == 0 || fragment_shader == 0 {
        return Err(anyhow!("Failed to make shader!"));
    }
    let program = glCreateProgram();
    glAttachShader(program, vertex_shader);
    glAttachShader(program, fragment_shader);

    glLinkProgram(program);
    glGetProgramiv(program, GL_LINK_STATUS, &mut program_ok);
    if program_ok == 0 {
        glDeleteShader(vertex_shader);
        glDeleteShader(fragment_shader);
        glDeleteProgram(program);
        return Err(anyhow!("Failed to link shader program"));
    }
    return Ok(program);
}

pub unsafe fn make_shader_object(shader_type: GLenum, source: &str) -> Result<GLuint, Error> {
    let string_ptr = source.as_ptr() as *const GLchar;
    let mut shader: GLuint = 0;
    let mut shader_ok: GLint = 0;
    shader = glCreateShader(shader_type);
    glShaderSource(shader, 1, &string_ptr as *const *const GLchar, null());
    glCompileShader(shader);
    glGetShaderiv(shader, GL_COMPILE_STATUS, &mut shader_ok);
    if shader_ok == 0 {
        glDeleteShader(shader);
        check_error().context("Failed to compile")?;
        return Err(anyhow!("Failed to compile!"));
    }
    return Ok(shader);
}

pub unsafe fn check_error() -> Result<(), anyhow::Error> {
    let err: GLenum = glGetError();
    match err {
        0 => return Ok(()),
        GL_INVALID_ENUM => Err(anyhow!("GL error: Invalid enum")),
        GL_INVALID_VALUE => Err(anyhow!("GL error: Invalid value")),
        GL_INVALID_OPERATION => Err(anyhow!("GL error: Invalid operation")),
        GL_OUT_OF_MEMORY => Err(anyhow!("GL error: Out of memory")),
        GL_INVALID_FRAMEBUFFER_OPERATION => Err(anyhow!("GL error: invalid frame buffer")),
        _ => Err(anyhow!("GL error: Unknown {:#x}", err))
    }
}

