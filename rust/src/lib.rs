#![cfg(target_os = "android")]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

extern crate android_logger;

use std::ffi::{c_void, CString};
use std::fs::File;
use std::io::{Read, Write};
use std::mem::size_of;
use std::os::raw::c_char;
use std::ptr::{null, null_mut};
use std::slice;

use anyhow::{anyhow, Context, Error};
use gles31_sys::{GL_COMPILE_STATUS, GL_COMPUTE_SHADER, GL_DYNAMIC_READ, GL_INFO_LOG_LENGTH, GL_MAP_READ_BIT, GL_READ_ONLY, GL_SHADER_STORAGE_BARRIER_BIT, GL_SHADER_STORAGE_BUFFER, GL_STREAM_COPY, glAttachShader, glBindBuffer, glBindBufferBase, glBufferData, GLchar, glCompileShader, glCreateProgram, glCreateShader, glDeleteShader, glDispatchCompute, glGenBuffers, glGetError, glGetShaderInfoLog, glGetShaderiv, GLint, glLinkProgram, glMapBufferRange, glMemoryBarrier, glShaderSource, GLsizei, GLuint, glUnmapBuffer, glUseProgram};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use khronos_egl::{choose_config, choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, create_pixmap_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, GL_COLORSPACE, GL_COLORSPACE_SRGB, initialize, make_current, NO_CONTEXT, query_surface, swap_buffers};
use log::error;
use log::info;
use log::Level;

#[no_mangle]
pub extern fn Java_com_mersive_glconvert_MainActivity_init(
    env: JNIEnv,
    _obj: JObject,
    path: JString,
) {
    android_logger::init_once(android_logger::Config::default().with_min_level(Level::Debug));
    info!("Hello, Rust!");

    let path: String = env.get_string(path).unwrap().into();
    let res = main(path);
    if res.is_err() {
        error!("Error running rust: {:?}", res.unwrap());
    } else {
        info!("Converted image!");
    }
}

fn main(path: String) -> Result<(), Error> {
    unsafe {
        gl_init().context("Couldn't init OpenGL!");

        // load input image
        let width = 1300;
        let height = 1300;
        let filename = format!("{}/thanksgiving.raw", path);
        let mut file = File::open(&filename).context("no file found")?;
        let metadata = std::fs::metadata(&filename).context("unable to read metadata")?;
        let mut data = vec![0; metadata.len() as usize];
        file.read(&mut data).context("buffer overflow")?;
        info!("Read {} byte image", data.len());

        // Extract Y plane
        let (out_byte_cnt, out_word_stride, yuy2_to_y8) = create_yuy2_to_y8(width, height)?;
        let _input_buffer = create_input_buffer(&data, 0)?;
        let _output_buffer = create_output_buffer(out_byte_cnt, 1)?;
        let y_plane = run_program(yuy2_to_y8, out_word_stride, height, out_byte_cnt);

        // Extract Y plane
        let (out_byte_cnt, out_word_stride, yuy2_to_uv) = create_yuy2_to_uv(width, height)?;
        let _input_buffer = create_input_buffer(&data, 0)?;
        let _output_buffer = create_output_buffer(out_byte_cnt, 1)?;
        let uv_plane = run_program(yuy2_to_uv, out_word_stride, height, out_byte_cnt);

        // Save
        let path = format!("{}/pic0.raw", path);
        info!("Writing file {}...", path);
        let mut file = File::create(path)?;
        file.write_all(&y_plane[..])?;
        file.write_all(&uv_plane[..])?;
    }
    Ok(())
}

fn gl_init() -> Result<(), Error> {
    let display = get_display(DEFAULT_DISPLAY).context("Need a display!")?;
    let res = initialize(display).context("Can't initialize")?;
    info!("EGL version={:?}", res);

    let config = choose_first_config(display, &[khronos_egl::NONE])
        .context("unable to choose an EGL configuration")?
        .ok_or(anyhow!("No available config!"))?;
    let attributes = [
        khronos_egl::CONTEXT_MAJOR_VERSION, 3,
        khronos_egl::CONTEXT_MINOR_VERSION, 1,
        khronos_egl::NONE
    ];
    let ctx = create_context(display, config, None, &attributes).context("Need a context!")?;
    make_current(display, None, None, Some(ctx)).expect("Can't make current");
    Ok(())
}

fn run_program<'a>(program: u32, x_sz: usize, y_sz: usize, out_byte_cnt: usize) -> &'a [u8] {
    unsafe {
        glUseProgram(program);
        glDispatchCompute(y_sz as u32, x_sz as u32, 1);
        glMemoryBarrier(GL_SHADER_STORAGE_BARRIER_BIT);
        let ptr = glMapBufferRange(GL_SHADER_STORAGE_BUFFER, 0, out_byte_cnt as i64, GL_MAP_READ_BIT) as *const u8;
        glUnmapBuffer(GL_SHADER_STORAGE_BUFFER);
        slice::from_raw_parts(ptr, out_byte_cnt as usize)
    }
}

fn create_input_buffer(mut data: &Vec<u8>, bind_idx: usize) -> Result<u32, Error> {
    let mut buf: GLuint = 0;
    unsafe {
        glGenBuffers(1, &mut buf);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, buf);
        glBufferData(GL_SHADER_STORAGE_BUFFER, data.len() as i64, data.as_ptr() as *const c_void, GL_STREAM_COPY); // TODO: STREAM_DRAW?
        glBindBufferBase(GL_SHADER_STORAGE_BUFFER, bind_idx as u32, buf);
    }
    if buf == 0 {
        return Err(anyhow!("Couldn't create input buffer!"));
    }
    Ok(buf)
}

fn create_output_buffer(length: usize, bind_idx: usize) -> Result<u32, Error> {
    let mut buf: GLuint = 0;
    unsafe {
        glGenBuffers(1, &mut buf);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, buf);
        glBufferData(GL_SHADER_STORAGE_BUFFER, length as i64, null() as *const c_void, GL_DYNAMIC_READ);
        glBindBufferBase(GL_SHADER_STORAGE_BUFFER, bind_idx as u32, buf);
    }
    if buf == 0 {
        return Err(anyhow!("Couldn't create output buffer!"));
    }
    Ok(buf)
}

fn create_yuy2_to_y8(width: usize, height: usize) -> Result<(usize, usize, u32), Error> {
    let in_px_per_word = 2;
    let out_px_per_word = 4;

    let out_px_cnt = (width * height) as usize; // Y plane only for now
    let out_word_cnt = out_px_cnt / size_of::<u32>();
    let out_byte_cnt = out_word_cnt * size_of::<u32>();
    let in_word_stride = width / in_px_per_word;
    let out_word_stride = width / out_px_per_word;

    // https://stackoverflow.com/questions/51245319/minimal-working-example-of-compute-shader-for-open-gl-es-3-1
    let COMPUTE_SHADER = format!("#version 310 es\n\
layout(local_size_x = 1, local_size_y = 1) in;\n\
layout(std430) buffer;\n\
layout(binding = 0) readonly buffer Input0 {{\n\
    uint elements[{height}][{in_word_stride}];\n\
}} input_data0;\n\
layout(binding = 1) writeonly buffer Output {{\n\
    uint elements[{height}][{out_word_stride}];\n\
}} output_data;\n\
void main() {{\n\
    uint out_px_per_word = {out_px_per_word}u;\n\
    uint y = gl_GlobalInvocationID.x;\n\
    uint x = gl_GlobalInvocationID.y * out_px_per_word;\n\
\n\
    uint out_word = 0u;\n\
    for(uint i = 0u; i < out_px_per_word; i++) {{\n\
        uint shift = (x + i) % 2u == 0u ? 0u : 16u;\n\
        uint Y = (input_data0.elements[y][(x + i) / 2u] >> shift) & 0xFFu;\n\
        out_word |= (Y << (i * 8u));\n\
    }}\n\
\n\
    output_data.elements[y][gl_GlobalInvocationID.y] = out_word;\n\
}}",
                                 height = height,
                                 in_word_stride = in_word_stride,
                                 out_word_stride = out_word_stride,
                                 out_px_per_word = out_px_per_word,
    );
    info!("y_shader={}", COMPUTE_SHADER);
    unsafe {
        let shader = load_shader(COMPUTE_SHADER.as_str())?;
        let program = glCreateProgram();
        glAttachShader(program, shader);
        glLinkProgram(program);
        Ok((out_byte_cnt, out_word_stride, program))
    }
}

fn create_yuy2_to_uv(in_width: usize, in_height: usize) -> Result<(usize, usize, u32), Error> {
    let scale = 2; // uv plane is 1/2 size of input
    let bytes_per_px = 2; // u & v
    let px_per_word = size_of::<GLuint>() / bytes_per_px;

    let out_width = in_width / scale;
    let out_height = in_height / scale;
    let out_px_cnt = (out_width * out_height) as usize;
    let out_byte_cnt = out_px_cnt * bytes_per_px;
    let in_word_stride = in_width / px_per_word;
    let out_word_stride = out_width / px_per_word;

    // https://stackoverflow.com/questions/51245319/minimal-working-example-of-compute-shader-for-open-gl-es-3-1
    let COMPUTE_SHADER = format!("#version 310 es\n\
layout(local_size_x = 1, local_size_y = 1) in;\n\
layout(std430) buffer;\n\
layout(binding = 0) readonly buffer Input0 {{\n\
    uint elements[{in_height}][{in_word_stride}];\n\
}} input_data0;\n\
layout(binding = 1) writeonly buffer Output {{\n\
    uint elements[{out_height}][{out_word_stride}];\n\
}} output_data;\n\
void main() {{\n\
    uint px_per_word = {px_per_word}u;\n\
    uint out_x = gl_GlobalInvocationID.y * px_per_word;\n\
    uint out_y = gl_GlobalInvocationID.x;\n\
    uint in_x = out_x;\n\
    uint in_y = out_y * 2u;\n\
\n\
    uint u1 = ((input_data0.elements[in_y][in_x + 0u] >> 8) & 0xFFu);\n\
    uint v1 = ((input_data0.elements[in_y][in_x + 0u] >> 24) & 0xFFu);\n\
    uint u2 = ((input_data0.elements[in_y][in_x + 1u] >> 8) & 0xFFu);\n\
    uint v2 = ((input_data0.elements[in_y][in_x + 1u] >> 24) & 0xFFu);\n\
    uint out_word = u1 | v1 << 8 | u2 << 16 | v2 << 24;\n\
\n\
    output_data.elements[out_y][gl_GlobalInvocationID.y] = out_word;\n\
}}",
                                 in_height = in_height,
                                 out_height = out_height,
                                 in_word_stride = in_word_stride,
                                 out_word_stride = out_word_stride,
                                 px_per_word = px_per_word,
    );
    info!("uv_shader={}", COMPUTE_SHADER);
    unsafe {
        let shader = load_shader(COMPUTE_SHADER.as_str())?;
        let program = glCreateProgram();
        glAttachShader(program, shader);
        glLinkProgram(program);
        Ok((out_byte_cnt, out_word_stride, program))
    }
}

pub unsafe fn load_shader(shader_src: &str) -> Result<GLuint, Error> {
    let shader = glCreateShader(GL_COMPUTE_SHADER);
    if shader == 0 {
        let err = glGetError();
        return Err(anyhow!("Error creating shader: {}", err));
    }
    let shader_src = format!("{}\0", shader_src).as_str().as_ptr() as *const GLchar;
    glShaderSource(shader, 1, &shader_src, null());
    glCompileShader(shader);

    let mut compiled: GLint = 0;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &mut compiled);
    if compiled == 0 {
        let mut info_len: GLint = 0;
        glGetShaderiv(shader, GL_INFO_LOG_LENGTH, &mut info_len);
        if info_len > 1 {
            let mut info_log = vec![0u8; info_len as usize];
            glGetShaderInfoLog(shader, info_len, null_mut() as *mut GLsizei, info_log.as_ptr() as *mut u8);
            let str = CString::new(info_log)?;
            return Err(anyhow!("Error compiling shader: {}", str.to_str()?));
        }
        glDeleteShader(shader);
        return Err(anyhow!("Error compiling shader!"));
    }
    return Ok(shader);
}
