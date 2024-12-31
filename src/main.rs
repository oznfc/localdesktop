use eframe::NativeOptions;
use polar_bear::DemoApp;

fn main() -> Result<(), eframe::Error> {
    let options = NativeOptions::default();
    DemoApp::run(options)
}
