use khronos_egl::{choose_first_config, Config, CONTEXT_CLIENT_VERSION, create_context, DEFAULT_DISPLAY, EGLConfig, get_current_display, get_display, initialize, make_current, NO_CONTEXT};

fn main() {

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

    // gles
    
}
