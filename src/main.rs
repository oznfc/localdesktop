use eframe::NativeOptions;
use polar_bear::app::ui::PolarBearApp;

fn main() -> Result<(), eframe::Error> {
    let mut options = NativeOptions::default();
    options.viewport = options.viewport.with_fullscreen(true);
    PolarBearApp::run(options)
}
