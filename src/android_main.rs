use winit::{
    event_loop::{ControlFlow, EventLoop},
    platform::android::{activity::AndroidApp, EventLoopBuilderExtAndroid},
};

use crate::{
    app::build::PolarBearApp,
    utils::{
        application_context::ApplicationContext,
        fullscreen_immersive::{enable_fullscreen_immersive_mode, keep_screen_on},
        logging::PolarBearExpectation,
        ndk::run_in_jvm,
    },
};

#[no_mangle]
fn android_main(android_app: AndroidApp) {
    ApplicationContext::build(&android_app);

    run_in_jvm(enable_fullscreen_immersive_mode, android_app.clone());
    run_in_jvm(keep_screen_on, android_app.clone());

    std::env::set_var("RUST_BACKTRACE", "full");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let event_loop = EventLoop::builder()
        .with_android_app(android_app.clone())
        .build()
        .pb_expect("Failed to create event loop");

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    // event_loop.set_control_flow(ControlFlow::Poll);

    // ControlFlow::Wait pauses the event loop if no events are available to process.
    // This is ideal for non-game applications that only update in response to user
    // input, and uses significantly less power/CPU time than ControlFlow::Poll.
    event_loop.set_control_flow(ControlFlow::Wait);

    // Phase 1: Setup
    let mut app = PolarBearApp::build(android_app);

    // Phase 2: Run
    event_loop.run_app(&mut app).pb_expect("Failed to run app");
}
