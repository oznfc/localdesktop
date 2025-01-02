use eframe::NativeOptions;
use polar_bear::PolarBearApp;

fn main() -> Result<(), eframe::Error> {
    let options = NativeOptions::default();
    PolarBearApp::run(options)
}
