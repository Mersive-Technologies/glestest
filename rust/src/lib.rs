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

use anyhow::{anyhow, Context, Error};
use gles31_sys::{GL_COMPILE_STATUS, GL_COMPUTE_SHADER, GL_INFO_LOG_LENGTH, GL_SHADER_STORAGE_BUFFER, GL_STREAM_COPY, glBindBuffer, glBindBufferBase, glBufferData, GLchar, glCompileShader, glCreateProgram, glCreateShader, glDeleteShader, glGenBuffers, glGetError, glGetShaderInfoLog, glGetShaderiv, GLint, glShaderSource, GLsizei, GLuint, glAttachShader, glLinkProgram, glUseProgram, glDispatchCompute, glMapBufferRange, GL_READ_ONLY, glUnmapBuffer, glMemoryBarrier, GL_SHADER_STORAGE_BARRIER_BIT, GL_MAP_READ_BIT, GL_DYNAMIC_READ};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use khronos_egl::{choose_config, choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, create_pixmap_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, GL_COLORSPACE, GL_COLORSPACE_SRGB, initialize, make_current, NO_CONTEXT, query_surface, swap_buffers};
use log::error;
use log::info;
use log::Level;
use std::slice;

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
        // let args: Vec<String> = env::args().collect();
        let idx = 0; //args.get(1).unwrap().parse::<usize>().unwrap();

        // egl init
        let display = get_display(DEFAULT_DISPLAY).context("Need a display!")?;
        let res = initialize(display).context("Can't initialize")?;
        info!("EGL version={:?}", res);
        let mut configs: Vec<Config> = Vec::with_capacity(100);
        info!("Choosing config...");
        let attributes = [
            khronos_egl::NONE
        ];
        choose_config(display, &attributes, &mut configs)
            .context("unable to choose an EGL configuration")?;
        info!("count={}", configs.len());
        let config = configs.remove(idx);
        let attributes = [
            khronos_egl::CONTEXT_MAJOR_VERSION, 3,
            khronos_egl::CONTEXT_MINOR_VERSION, 1,
            khronos_egl::NONE
        ];
        let ctx = create_context(display, config, None, &attributes).context("Need a context!")?;
        info!("EGL context={:?}", ctx);

        // create surface
        let width = 1300;
        let height = 1300;
        let mut attributes = vec![
            khronos_egl::WIDTH, width.clone(),
            khronos_egl::HEIGHT, height.clone(),
        ];
        #[cfg(os = "linux")]
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
        info!("w={} h={}", w, h);

        // https://github.com/AlexCharlton/hello-modern-opengl/blob/master/hello-gl.c
        #[cfg(target_os = "android")]
            let ver = "300 es";
        #[cfg(target_os = "linux")]
            let ver = "330";

        info!("ver={}", ver);

        // https://stackoverflow.com/questions/51245319/minimal-working-example-of-compute-shader-for-open-gl-es-3-1
        let COMPUTE_SHADER = "#version 310 es\n\
layout(local_size_x = 1, local_size_y = 1) in;\n\
layout(std430) buffer;\n\
layout(binding = 0) writeonly buffer Output {\n\
    uint elements[1300][1300];\n\
} output_data;\n\
layout(binding = 1) readonly buffer Input0 {\n\
    uint elements[1300][650];\n\
} input_data0;\n\
void main()\n\
{\n\
    uint y = gl_GlobalInvocationID.x;\n\
    uint x = gl_GlobalInvocationID.y;\n\

    uint shift = x % 2u == 0u ? 0u : 16u;
    uint Y = (input_data0.elements[y][x / 2u] >> shift) & 0xFFu;\n\
    uint argb = 255u << 24u | Y << 16u | Y << 8u | Y;

    output_data.elements[y][x] = argb;\n\
}";

        // texture
        let filename = format!("{}/thanksgiving.raw", path);
        let mut f = File::open(&filename).context("no file found")?;
        let metadata = std::fs::metadata(&filename).context("unable to read metadata")?;
        let mut data = vec![0; metadata.len() as usize];
        f.read(&mut data).context("buffer overflow")?;
        info!("Read {} byte image", data.len());

        let in_byte_cnt = data.len();
        let mut input_buffer: GLuint = 0;
        glGenBuffers(1, &mut input_buffer);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, input_buffer);
        glBindBufferBase(GL_SHADER_STORAGE_BUFFER, 1, input_buffer);
        glBufferData(GL_SHADER_STORAGE_BUFFER, in_byte_cnt as i64, data.as_ptr() as *const c_void, GL_STREAM_COPY);
        info!("input_buffer worked!");

        let out_px_cnt = (width * height) as usize; // Y plane only for now
        let out_byte_cnt = out_px_cnt * size_of::<u32>();
        let mut output_buffer: GLuint = 0;
        glGenBuffers(1, &mut output_buffer);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, output_buffer);
        glBindBufferBase(GL_SHADER_STORAGE_BUFFER, 0, output_buffer);
        glBufferData(GL_SHADER_STORAGE_BUFFER, out_byte_cnt as i64, null() as *const c_void, GL_DYNAMIC_READ);
        info!("output_buffer worked!");

        let program = glCreateProgram();
        let shader = load_shader(COMPUTE_SHADER)?;
        info!("load_shader worked!");

        glAttachShader(program, shader);
        glLinkProgram(program);
        glUseProgram(program);
        glDispatchCompute(width as u32, height as u32, 1);
        info!("glDispatchCompute worked!");

        let ptr = glMapBufferRange(GL_SHADER_STORAGE_BUFFER, 0, out_byte_cnt as i64, GL_MAP_READ_BIT ) as *const u8;
        info!("glMapBufferRange worked: {:?}", ptr);
        glUnmapBuffer( GL_SHADER_STORAGE_BUFFER );
        info!("glUnmapBuffer worked!");
        let pixels = slice::from_raw_parts(ptr, out_byte_cnt as usize);

        // Save
        let path = format!("{}/pic{}.raw", path, idx);
        info!("Writing file {}...", path);
        let mut file = File::create(path)?;
        file.write_all(&pixels[..])?;

        glMemoryBarrier( GL_SHADER_STORAGE_BARRIER_BIT );
    }
    Ok(())
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
