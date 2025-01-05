use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
};

use arch::{arch_run, arch_run_with_log};
use eframe::{egui, NativeOptions};

pub mod arch;
pub mod config;
pub mod logging;
pub mod wayland;

#[cfg(target_os = "android")]
use egui_winit::winit;
use logging::{log_format, log_to_panel, PolarBearExpectation};
use wayland::PolarBearCompositor;

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
    logs: Arc<Mutex<VecDeque<String>>>,
    compositor: Arc<PolarBearCompositor>,
}

impl PolarBearApp {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let compositor = Arc::new(PolarBearCompositor {});
        let app = PolarBearApp {
            logs: Arc::clone(&logs),
            compositor: Arc::clone(&compositor),
        };
        thread::spawn(move || {
            arch_run_with_log(&["uname", "-a"], &logs);
            loop {
                let installed = arch_run(&["pacman", "-Qg", "plasma"])
                    .wait()
                    .pb_expect("pacman -Qg plasma failed")
                    .success();
                if installed {
                    match compositor.run() {
                        Ok(_) => {
                            log_to_panel(
                                &log_format(
                                    "POLAR BEAR COMPOSITOR STARTED",
                                    "Polar Bear Compositor started successfully",
                                ),
                                &logs,
                            );
                        }
                        Err(e) => {
                            log_to_panel(
                                &log_format(
                                    "POLAR BEAR COMPOSITOR RUNTIME ERROR",
                                    &format!("{}", e),
                                ),
                                &logs,
                            );
                        }
                    }
                    arch_run_with_log(&["weston"], &logs);
                    break;
                } else {
                    arch_run(&["rm", "/var/lib/pacman/db.lck"]);
                    arch_run_with_log(
                        &["pacman", "-Syu", "plasma", "weston", "--noconfirm"],
                        &logs,
                    );
                }
            }
        });
        eframe::run_native("Polar Bear", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

impl eframe::App for PolarBearApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::right("log_panel")
            .resizable(true)
            .default_width(320.0)
            .width_range(80.0..=ctx.available_rect().width() / 2.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Logs");
                });
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        let logs = self.logs.lock().unwrap();
                        ui.label(logs.iter().cloned().collect::<Vec<_>>().join("\n"))
                    });
            });
    }
}
