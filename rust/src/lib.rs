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
use gles31_sys::{GL_COMPILE_STATUS, GL_COMPUTE_SHADER, GL_DYNAMIC_READ, GL_INFO_LOG_LENGTH, GL_MAP_READ_BIT, GL_READ_ONLY, GL_SHADER_STORAGE_BARRIER_BIT, GL_SHADER_STORAGE_BUFFER, GL_STREAM_COPY, glAttachShader, glBindBuffer, glBindBufferBase, glBufferData, GLchar, glCompileShader, glCreateProgram, glCreateShader, glDeleteShader, glDispatchCompute, glGenBuffers, glGetError, glGetShaderInfoLog, glGetShaderiv, GLint, glLinkProgram, glMapBufferRange, glMemoryBarrier, glShaderSource, GLsizei, GLuint, glUnmapBuffer, glUseProgram, glLineWidth, GL_DYNAMIC_DRAW, glFinish};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use khronos_egl::{choose_config, choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, create_pixmap_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, GL_COLORSPACE, GL_COLORSPACE_SRGB, initialize, make_current, NO_CONTEXT, query_surface, swap_buffers, EGLContext, destroy_context, Display};
use log::error;
use log::info;
use log::Level;
use std::time::Instant;
use std::cmp::{min, max};
use rand::Rng;

#[no_mangle]
pub extern fn Java_com_mersive_glconvert_MainActivity_init(
    env: JNIEnv,
    _obj: JObject,
    path: JString,
) {
    android_logger::init_once(android_logger::Config::default().with_min_level(Level::Debug));
    info!("Hello, Rust!");

    let path: String = env.get_string(path).unwrap().into();


    let filename = format!("{}/thanksgiving.raw", path);
    let mut file = File::open(&filename).context("no file found").unwrap();
    let metadata = std::fs::metadata(&filename).context("unable to read metadata").unwrap();

    // load input image
    let mut data = vec![0; metadata.len() as usize];
    file.read(&mut data).context("buffer overflow").unwrap();
    info!("Read {} byte image", data.len());


    let converter = GlColorConverter::new(1300, 1300, &data).unwrap();

    let size = 100;
    let mut d = Vec::with_capacity(size);
    for _i in 0..size {
        let start = Instant::now();
        let out = converter.convert_frame(&data).unwrap();
        unsafe {
            glFinish();
        }
        let duration = start.elapsed().as_micros();
        d.push(duration as u64);
    }
    let mean = d.iter().fold(0f64, |acc, &cur| acc + cur as f64) / size as f64;
    let dev = (d.iter().fold(0f64, |acc, &cur| (cur as f64 - mean).powf(2f64) + acc) / size as f64).sqrt();
    let sorted = d.sort();
    let median = d[size / 2];
    let min = d.iter().fold(u64::max_value(), |acc, &cur| min(acc, cur));
    let max = d.iter().fold(0u64, |acc, &cur| max(acc, cur));

    info!("mean {} stddev {} median {} min {} max {}", mean, dev, median, min, max);


    let mut rng = rand::thread_rng();
    let r = rng.gen_range(0,1000);
    // Save
    let out = converter.convert_frame(&data).unwrap();
    let path = format!("{}/pic{}.raw", path, r);
    info!("Writing file {}...", path);
    let mut file = File::create(path).unwrap();
    file.write_all(&out).unwrap();
}

struct Nv12SizeInfo {
    width: usize,
    height: usize,
}

impl Nv12SizeInfo {
    fn y_in_px_per_word(&self) -> usize { 2 }
    fn y_out_px_per_word(&self) -> usize { 4 }
    fn y_out_px_cnt(&self) -> usize { (self.width * self.height) as usize }
    fn y_out_word_cnt(&self) -> usize { self.y_out_px_cnt() / size_of::<u32>() }
    fn y_out_byte_cnt(&self) -> usize { self.y_out_word_cnt() * size_of::<u32>() }
    fn y_in_word_stride(&self) -> usize { self.width / self.y_in_px_per_word() }
    fn y_out_word_stride(&self) -> usize { self.width / self.y_out_px_per_word() }

    // uv plane is 1/2 size of input
    fn uv_scale(&self) -> usize { 2 }
    fn uv_bytes_per_px(&self) -> usize { 2 }
    fn uv_px_per_word(&self) -> usize { size_of::<GLuint>() / self.uv_bytes_per_px() }
    fn uv_out_width(&self) -> usize { self.width / self.uv_scale() }
    fn uv_out_height(&self) -> usize { self.height / self.uv_scale() }
    fn uv_out_px_cnt(&self) -> usize { (self.uv_out_width() * self.uv_out_height()) as usize }
    fn uv_out_byte_cnt(&self) -> usize { self.uv_out_px_cnt() * self.uv_bytes_per_px() }
    fn uv_in_word_stride(&self) -> usize { self.width / self.uv_px_per_word() }
    fn uv_out_word_stride(&self) -> usize { self.uv_out_width() / self.uv_px_per_word() }

    fn total_byte_cnt(&self) -> usize { self.y_out_byte_cnt() + self.uv_out_byte_cnt() }
}

struct GlColorConverter {
    width: usize,
    height: usize,
    ctx: Option<khronos_egl::Context>,
    display: Option<Display>,
    yuy2_to_y8_program: GLuint,
    yuy2_to_uv_program: GLuint,
    pub y_input_buffer: GLuint,
    y_output_buffer: GLuint,
    pub uv_input_buffer: GLuint,
    uv_output_buffer: GLuint,
    local_size_x: usize,
    local_size_y: usize,
}

impl GlColorConverter {
    pub fn new(width: usize, height: usize, src_frame: &Vec<u8>) -> Result<GlColorConverter, Error> {
        let (display, ctx) = gl_init().context("Couldn't init OpenGL!")?;

        let size_info = Nv12SizeInfo{width, height};

        let local_size_x = GlColorConverter::greatest_pow2_divisor(width);
        let local_size_y = GlColorConverter::greatest_pow2_divisor(height);
        let ret = GlColorConverter {
            width,
            height,
            ctx: Some(ctx),
            display: Some(display),
            yuy2_to_y8_program: create_yuy2_to_y8(width, height, local_size_x, local_size_y)?,
            y_input_buffer: create_input_buffer(0)?,
            y_output_buffer: create_output_buffer(1, size_info.y_out_byte_cnt())?,
            yuy2_to_uv_program: create_yuy2_to_uv(width, height, local_size_x, local_size_y)?,
            uv_input_buffer: create_input_buffer(0)?,
            uv_output_buffer: create_output_buffer(1, size_info.uv_out_byte_cnt())?,
            local_size_x,
            local_size_y,
        };

        upload_input_buffer(ret.y_input_buffer, &src_frame);
        upload_input_buffer(ret.uv_input_buffer, &src_frame);

        Ok(ret)
    }

    fn greatest_pow2_divisor(num: usize) -> usize { (num & (!(num-1))) }

    pub fn convert_frame(&self, src_frame: &Vec<u8>) -> Result<Vec<u8>, Error> {
        let size_info = Nv12SizeInfo { width: self.width, height: self.height };

        let mut output_frame = Vec::with_capacity(size_info.total_byte_cnt());

        // Extract Y plane
        // upload_input_buffer(self.y_input_buffer, &src_frame);
        self.run_program(self.yuy2_to_y8_program, self.y_input_buffer, 0, self.y_output_buffer, 1, size_info.y_out_word_stride(), &mut output_frame, size_info.y_out_byte_cnt());

        // Extract UV plane
        // upload_input_buffer(self.uv_input_buffer, &src_frame);
        self.run_program(self.yuy2_to_uv_program, self.uv_input_buffer, 0, self.uv_output_buffer, 1, size_info.uv_out_word_stride(), &mut output_frame, size_info.uv_out_byte_cnt());

        Ok(output_frame)
    }

    fn run_program(&self, program: u32, input_buffer: GLuint, input_bind_idx: GLuint, output_buffer: GLuint, output_bind_idx: GLuint, x_sz: usize, out: &mut Vec<u8>, out_size: usize) {
        unsafe {
            glBindBuffer(GL_SHADER_STORAGE_BUFFER, input_buffer); // TODO 1
            glBindBufferBase(GL_SHADER_STORAGE_BUFFER, input_bind_idx, input_buffer);
            glBindBuffer(GL_SHADER_STORAGE_BUFFER, output_buffer);
            glBindBufferBase(GL_SHADER_STORAGE_BUFFER, output_bind_idx, output_buffer);
            glUseProgram(program);
            glDispatchCompute((self.height / self.local_size_y) as u32, (x_sz / self.local_size_x) as u32, 1);
            glMemoryBarrier(GL_SHADER_STORAGE_BARRIER_BIT);
            let ptr = glMapBufferRange(GL_SHADER_STORAGE_BUFFER, 0, out_size as i64, GL_MAP_READ_BIT) as *const u8;
            let slice = slice::from_raw_parts(ptr, out_size);
            out.extend_from_slice(slice);
            glUnmapBuffer(GL_SHADER_STORAGE_BUFFER);
        }
    }
}

impl Drop for GlColorConverter {
    fn drop(&mut self) {
        // TODO: teardown opengl stuff
        if let Some(ctx) = self.ctx {
            let display = self.display.expect("Display should always exist if ctx does");
            destroy_context(display, ctx);
        }
    }
}

fn gl_init() -> Result<(Display, khronos_egl::Context), Error> {
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
    make_current(display, None, None, Some(ctx.clone())).expect("Can't make current");
    Ok((display, ctx))
}

fn create_input_buffer(bind_idx: GLuint) -> Result<u32, Error> {
    let mut buf: GLuint = 0;
    unsafe {
        glGenBuffers(1, &mut buf);
    }
    if buf == 0 {
        return Err(anyhow!("Couldn't create input buffer!"));
    }
    Ok(buf)
}

fn upload_input_buffer(buffer_id: GLuint, data: &Vec<u8>) -> () {
    unsafe {
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, buffer_id);
        glBufferData(GL_SHADER_STORAGE_BUFFER, data.len() as i64, data.as_ptr() as *const c_void, GL_STREAM_COPY); // TODO GL_DYNAMIC_DRAW
    }
}

fn create_output_buffer(bind_idx: GLuint, size: usize) -> Result<u32, Error> {
    let mut buf: GLuint = 0;
    unsafe {
        glGenBuffers(1, &mut buf);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, buf);
        glBufferData(GL_SHADER_STORAGE_BUFFER, size as i64, null() as *const c_void, GL_DYNAMIC_READ);
    }
    if buf == 0 {
        return Err(anyhow!("Couldn't create output buffer!"));
    }
    Ok(buf)
}

fn create_yuy2_to_y8(width: usize, height: usize, local_size_x: usize, local_size_y: usize) -> Result<u32, Error> {
    let size_info = Nv12SizeInfo { width, height };

    // https://stackoverflow.com/questions/51245319/minimal-working-example-of-compute-shader-for-open-gl-es-3-1
    let COMPUTE_SHADER = format!("#version 310 es\n\
layout(local_size_x = {local_size_x}, local_size_y = {local_size_y}) in;\n\
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
                                 height = size_info.height,
                                 in_word_stride = size_info.y_in_word_stride(),
                                 out_word_stride = size_info.y_out_word_stride(),
                                 out_px_per_word = size_info.y_out_px_per_word(),
                                 local_size_x = local_size_x,
                                 local_size_y = local_size_y,
    );
    info!("y_shader={}", COMPUTE_SHADER);
    unsafe {
        let shader = load_shader(COMPUTE_SHADER.as_str())?;
        let program = glCreateProgram();
        glAttachShader(program, shader);
        glLinkProgram(program);
        glDeleteShader(shader);
        Ok(program)
    }
}

fn create_yuy2_to_uv(width: usize, height: usize, local_size_x: usize, local_size_y: usize) -> Result<u32, Error> {
    let size_info = Nv12SizeInfo { width, height };

    // https://stackoverflow.com/questions/51245319/minimal-working-example-of-compute-shader-for-open-gl-es-3-1
    let COMPUTE_SHADER = format!("#version 310 es\n\
layout(local_size_x = {local_size_x}, local_size_y = {local_size_y}) in;\n\
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
                                 in_height = size_info.height,
                                 out_height = size_info.uv_out_height(),
                                 in_word_stride = size_info.uv_in_word_stride(),
                                 out_word_stride = size_info.uv_out_word_stride(),
                                 px_per_word = size_info.uv_px_per_word(),
                                 local_size_x = local_size_x,
                                 local_size_y = local_size_y,
    );
    info!("uv_shader={}", COMPUTE_SHADER);
    unsafe {
        let shader = load_shader(COMPUTE_SHADER.as_str())?;
        let program = glCreateProgram();
        glAttachShader(program, shader);
        glLinkProgram(program);
        Ok(program)
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
