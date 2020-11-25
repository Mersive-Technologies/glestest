use khronos_egl::{choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, initialize, make_current, NO_CONTEXT};
use rs_gles3::{glGenBuffers, GLuint, glGenVertexArrays, glBindVertexArray, glBindBuffer, GL_ARRAY_BUFFER, glBufferData, GL_STATIC_DRAW, glEnableVertexAttribArray, glVertexAttribPointer, GL_FLOAT, GL_FALSE, GLfloat, glClear, GL_COLOR_BUFFER_BIT, GLenum, GLint, glCreateShader, glShaderSource, glCompileShader, glGetShaderiv, GL_COMPILE_STATUS, glDeleteShader, GLchar, GL_VERTEX_SHADER, GL_FRAGMENT_SHADER, glCreateProgram, glAttachShader, glLinkProgram, glGetProgramiv, GL_LINK_STATUS, glDeleteProgram, glDetachShader, glGetError, GL_INVALID_ENUM, GL_INVALID_VALUE, GL_INVALID_OPERATION, GL_OUT_OF_MEMORY, GL_INVALID_FRAMEBUFFER_OPERATION};
use std::ptr::null;
use std::ffi::c_void;
use anyhow::{anyhow, Context, Error};

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
        make_current(display, None, None, Some(ctx)).expect("Can't make current");

        // https://github.com/AlexCharlton/hello-modern-opengl/blob/master/hello-gl.c
        let mut vertex_buffer_data = [
            -1.0f32, -1.0f32, -1.0f32, 0.0f32, 0.0f32, 1.0f32,
            1.0f32, -1.0f32, -1.0f32, 0.0f32, 1.0f32, 0.0f32,
            1.0f32,  1.0f32, -1.0f32, 1.0f32, 0.0f32, 0.0f32
        ];

        let mut vertex_array_id: GLuint = 0;
        glGenVertexArrays(1, &mut vertex_array_id);
        if vertex_array_id == 0 { panic!("Invalid vertex array!"); }

        let mut vertex_buffer: GLuint = 0;
        glGenBuffers(1, &mut vertex_buffer);
        if vertex_buffer == 0 { panic!("Invalid vertex buffer!"); }

        let mut index_buffer: GLuint = 0;
        glGenBuffers(1, &mut index_buffer);
        if index_buffer == 0 { panic!("Invalid index buffer!"); }

        glBindVertexArray(vertex_array_id);

        glBindBuffer(GL_ARRAY_BUFFER, vertex_buffer);
        check_error().context("Cannot bind buffer!")?;
        glBufferData(GL_ARRAY_BUFFER, (vertex_buffer_data.len() * 4usize) as i64, vertex_buffer_data.as_mut_ptr() as *const c_void, GL_STATIC_DRAW);
        check_error().context("Cannot set buffer data")?;

        glEnableVertexAttribArray(0); // 262
        glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as u8, 24, null());
        check_error().context("Cannot set vertex attrib pointer")?;

        glEnableVertexAttribArray(1);
        check_error().context("Cannot enable vertex attrib array")?;

        let num = 12;
        glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE as u8, 24, num as *const c_void);
        check_error().context("Cannot set vertex attrib pointer 2")?;

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

