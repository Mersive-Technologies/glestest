use khronos_egl::{create_context, get_current_display, NO_CONTEXT, CONTEXT_CLIENT_VERSION, Config, EGLConfig, DEFAULT_DISPLAY, get_display, initialize, choose_first_config};

fn main() {
    let display = get_display(DEFAULT_DISPLAY).expect("Need a display!");
    let attributes = [khronos_egl::NONE];
    let res = initialize(display).expect("Can't initialize");
    println!("EGL version={:?}", res);
    let config = choose_first_config(display, &attributes)
        .expect("unable to choose an EGL configuration")
        .expect("no EGL configuration found");
    let ctx = create_context(display, config, None, &attributes).expect("Need a context!");
    println!("Hello, world: {:?}", ctx);
}
