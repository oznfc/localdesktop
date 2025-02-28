use crate::app::run::PolarBearApp;
use eframe::{NativeOptions, Renderer};
use egui_winit::winit::platform::android::activity::AndroidApp;

#[no_mangle]
fn android_main(app: AndroidApp) {
    std::env::set_var("RUST_BACKTRACE", "full");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let options = NativeOptions {
        android_app: Some(app),
        renderer: Renderer::Glow,
        ..Default::default()
    };
    PolarBearApp::run(options).unwrap();
}
