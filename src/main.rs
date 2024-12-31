use eframe::NativeOptions;
use polar_bear::DemoApp;

fn main() -> Result<(), eframe::Error> {
    polar_bear::arch::boot();
    let options = NativeOptions::default();
    DemoApp::run(options)
}
