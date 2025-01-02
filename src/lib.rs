use std::{
    io::{BufRead, BufReader},
    process::Child,
    thread,
};

use arch::arch_run;
use eframe::{egui, NativeOptions};

pub mod arch;

#[cfg(target_os = "android")]
use egui_winit::winit;
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use eframe::Renderer;

    std::env::set_var("RUST_BACKTRACE", "full");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let options = NativeOptions {
        android_app: Some(app),
        renderer: Renderer::Wgpu,
        ..Default::default()
    };
    PolarBearApp::run(options).unwrap();
}

pub struct PolarBearApp {
    arch: Child,
    logs: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl PolarBearApp {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        let arch = arch_run(&["uname", "-a"]).unwrap();
        let logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut app = PolarBearApp {
            arch,
            logs: logs.clone(),
        };
        let stdout = app.arch.stdout.take().unwrap();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let line = line.unwrap();
                let mut logs = logs.lock().unwrap();
                logs.push(line);
            }
        });
        eframe::run_native("Polar Bear", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

impl eframe::App for PolarBearApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::right("log_panel")
            .resizable(true)
            .default_width(150.0)
            .width_range(80.0..=200.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Logs");
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let logs = self.logs.lock().unwrap();
                    ui.label(logs.join("\n"));
                });
            });
    }
}
