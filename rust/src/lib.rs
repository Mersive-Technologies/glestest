#![cfg(target_os = "android")]
#![allow(non_snake_case)]
#![allow(unused_imports)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

extern crate android_logger;

use std::ffi::c_void;
use std::fs::File;
use std::io::{Read, Write};
use std::os::raw::c_char;
use std::ptr::null;

use anyhow::{anyhow, Context, Error};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use khronos_egl::{choose_config, choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, create_pbuffer_surface, create_pixmap_surface, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, GL_COLORSPACE, GL_COLORSPACE_SRGB, initialize, make_current, NO_CONTEXT, query_surface, swap_buffers};
use log::error;
use log::info;
use log::Level;
use std::mem::size_of;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

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

        let vert_shader = format!("\
#version {}\n\
in vec3 vertex;\n\
in vec2 texcoordin;\n\
out vec2 texcoord;
void main(){{\n\
    gl_Position = vec4(vertex, 1.0);\n\
    texcoord = texcoordin;
}}\n\0", ver);

        let frag_shader = format!("\
#version {}\n\
out float color;\n\
uniform sampler2D tex_in;\n\
in vec2 texcoord;
void main(){{\n\
    color = texture(tex_in, texcoord).r;\n\
}}\n\0", ver);

        info!("{} {}", vert_shader, frag_shader);

        let mut data_buffer: GLuint = 0;
        let data: Vec<u32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        glGenBuffers(1, &mut data_buffer);
        glBindBuffer(GL_SHADER_STORAGE_BUFFER, data_buffer);
        glBufferData(GL_SHADER_STORAGE_BUFFER, size_of::<u32>() as i64 * 10, data.as_ptr() as *const c_void, GL_STREAM_COPY);
        info!("it worked!");
    }
    Ok(())
}
