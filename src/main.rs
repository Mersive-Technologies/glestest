use std::ffi::c_void;
use std::os::raw::c_char;
use std::ptr::null;

use anyhow::{anyhow, Context, Error};
use khronos_egl::{choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, initialize, make_current, NO_CONTEXT, GL_COLORSPACE, GL_COLORSPACE_SRGB};
use rs_gles3::{GL_ARRAY_BUFFER, GL_COLOR_BUFFER_BIT, GL_COMPILE_STATUS, GL_ELEMENT_ARRAY_BUFFER, GL_FALSE, GL_FLOAT, GL_FRAGMENT_SHADER, GL_INVALID_ENUM, GL_INVALID_FRAMEBUFFER_OPERATION, GL_INVALID_OPERATION, GL_INVALID_VALUE, GL_LINK_STATUS, GL_OUT_OF_MEMORY, GL_STATIC_DRAW, GL_TRIANGLES, GL_TRUE, GL_UNSIGNED_SHORT, GL_VERTEX_SHADER, glAttachShader, glBindBuffer, glBindVertexArray, glBufferData, GLchar, glClear, glCompileShader, glCreateProgram, glCreateShader, glDeleteProgram, glDeleteShader, glDetachShader, glDrawElements, glDrawElementsInstanced, glEnableVertexAttribArray, GLenum, GLfloat, glGenBuffers, glGenVertexArrays, glGetError, glGetProgramiv, glGetShaderiv, glGetUniformLocation, GLint, glLinkProgram, glReadPixels, glShaderSource, GLuint, glUniformMatrix4fv, glUseProgram, glVertexAttribPointer, glCopyTexImage2D, GL_RGB_INTEGER, GL_RGBA_INTEGER, GL_UNSIGNED_BYTE, glFinish, glPixelStorei, GL_UNPACK_ALIGNMENT, glBindFramebuffer, glViewport, glClearColor, GL_RGBA};
use std::fs::File;
use std::io::Write;

fn main() -> Result<(), Error> {
    unsafe {
        // egl init
        let display = get_display(DEFAULT_DISPLAY).expect("Need a display!");
        let attributes = [khronos_egl::NONE];
        let res = initialize(display).expect("Can't initialize");
        println!("EGL version={:?}", res);
        let config = choose_first_config(display, &attributes)
            .expect("unable to choose an EGL configuration")
            .expect("no EGL configuration found");
        let ctx = create_context(display, config, None, &attributes).expect("Need a context!");
        println!("EGL context={:?}", ctx);

        // create surface
        let width = 800;
        let height = 600;
        let attributes = [
            khronos_egl::WIDTH, width.clone(),
            khronos_egl::HEIGHT, height.clone(),
            khronos_egl::TEXTURE_FORMAT, khronos_egl::TEXTURE_RGBA,
            khronos_egl::TEXTURE_TARGET, khronos_egl::TEXTURE_2D,
            khronos_egl::NONE,
        ];
        let surface = create_pbuffer_surface(display, config, &attributes).expect("Couldn't create pbuffer");
        make_current(display, Some(surface.clone()), Some(surface.clone()), Some(ctx)).expect("Can't make current");

        // https://github.com/AlexCharlton/hello-modern-opengl/blob/master/hello-gl.c
        let vert_shader = "\
#version 300 es\n\
in vec3 vertex;\n\
in vec3 color;\n\
uniform mat4 mvp;\n\
out vec3 c;\n\
void main(){\n\
    gl_Position = mvp * vec4(vertex, 1.0);\n\
    c = color;\n\
}\n\0";

        let frag_shader = "\
#version 300 es\n\
in vec3 c;\n\
out vec4 color;\n\
void main(){\n\
    color = vec4(c, 1.0);\n\
}\0";

        let program_id = make_shader(vert_shader, frag_shader)?;
        let mvp_location = glGetUniformLocation(program_id, "mvp".as_ptr() as *const i8);

        let mut vertex_buffer_data = [
            -1.0f32, -1.0f32, -1.0f32, 0.0f32, 0.0f32, 1.0f32,
            1.0f32, -1.0f32, -1.0f32, 0.0f32, 1.0f32, 0.0f32,
            1.0f32, 1.0f32, -1.0f32, 1.0f32, 0.0f32, 0.0f32
        ];

        // 250
        let mut vertex_array_id: GLuint = 0;
        glGenVertexArrays(1, &mut vertex_array_id);
        if vertex_array_id == 0 { panic!("Invalid vertex array!"); }
        let mut vertex_buffer: GLuint = 0;
        glGenBuffers(1, &mut vertex_buffer);
        if vertex_buffer == 0 { panic!("Invalid vertex buffer!"); }
        let mut index_buffer: GLuint = 0;
        glGenBuffers(1, &mut index_buffer);
        if index_buffer == 0 { panic!("Invalid index buffer!"); }

        // 257
        glBindVertexArray(vertex_array_id);
        glBindBuffer(GL_ARRAY_BUFFER, vertex_buffer);
        check_error().context("Cannot bind buffer!")?;
        glBufferData(GL_ARRAY_BUFFER, (vertex_buffer_data.len() * 4) as i64, vertex_buffer_data.as_ptr() as *const c_void, GL_STATIC_DRAW);
        check_error().context("Cannot set buffer data")?;

        // 261
        glEnableVertexAttribArray(0); // 262
        glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as u8, 24, null());
        check_error().context("Cannot set vertex attrib pointer")?;
        glEnableVertexAttribArray(1);
        check_error().context("Cannot enable vertex attrib array")?;
        let num = 12;
        glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE as u8, 24, num as *const c_void);
        check_error().context("Cannot set vertex attrib pointer 2")?;

        // 267
        let index_buffer_data = [0u16, 1u16, 2u16];
        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, index_buffer);
        glBufferData(GL_ELEMENT_ARRAY_BUFFER, (index_buffer_data.len() * 2) as i64, index_buffer_data.as_ptr() as *const c_void, GL_STATIC_DRAW);
        check_error().context("Cannot bind index buffer")?;

        // 309
        // glClear(GL_COLOR_BUFFER_BIT);
        // check_error().context("Cannot clear!")?;

        // 277
        let mvp = [
            1f32, 0f32, 0f32, 0f32,
            0f32, 1f32, 0f32, 0f32,
            0f32, 0f32, 1f32, 0f32,
            0f32, 0f32, 0f32, 1f32,
        ];
        glUseProgram(program_id);
        check_error().context("Cannot use shader!")?;
        glUniformMatrix4fv(mvp_location, 1, GL_TRUE as u8, mvp.as_ptr() as *const f32);
        check_error().context("Cannot set MVP!")?;
        glBindVertexArray(vertex_array_id);
        check_error().context("Cannot bind vert array!")?;

        glViewport(0, 0, width, height);
        glClearColor(1.0, 0.0, 0.0, 1.0);
        glClear(GL_COLOR_BUFFER_BIT);
        check_error().context("Cannot clear!")?;

        glDrawElements(GL_TRIANGLES, 3, GL_UNSIGNED_SHORT, null());
        check_error().context("Cannot draw!")?;
        glFinish();
        check_error().context("Cannot finish!")?;

        let mut pixels = vec![0u8; (width * height) as usize * 4];
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        check_error().context("Cannot set pixel store mode!")?;
        glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut c_void);
        check_error().context("Cannot get pixels!")?;

        // Save
        let mut file = File::create("/data/pic.raw")?;
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
    glDetachShader(program, vertex_shader);
    glDetachShader(program, fragment_shader);
    glDeleteShader(vertex_shader);
    glDeleteShader(fragment_shader);
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

