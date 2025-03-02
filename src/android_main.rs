use winit::platform::android::activity::AndroidApp;

use crate::app::compositor::run_winit;

#[no_mangle]
fn android_main(app: AndroidApp) {
    std::env::set_var("RUST_BACKTRACE", "full");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    run_winit();
}
