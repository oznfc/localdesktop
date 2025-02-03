use crate::{
    app::{compositor::PolarBearCompositor, renderer::PolarBearRenderer},
    arch::run::{arch_run, arch_run_with_log},
    utils::{
        config,
        logging::{log_format, PolarBearExpectation},
    },
};
use eframe::{egui, NativeOptions};
use std::{
    collections::VecDeque,
    panic,
    sync::{Arc, Mutex},
    thread,
};

pub struct Shared {
    compositor: Option<PolarBearCompositor>,
    ctx: Option<egui::Context>,
    logs: VecDeque<String>,
}

impl Shared {
    pub fn log(&mut self, content: String) {
        self.logs.push_back(content);
        // Ensure the logs size stays at most 20
        if self.logs.len() > config::MAX_PANEL_LOG_ENTRIES {
            self.logs.pop_front();
        }
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }
}

pub struct PolarBearApp {
    shared: Arc<Mutex<Shared>>,
}

#[cfg(target_os = "android")]
use crate::{arch::scaffold, utils::application_context::ApplicationContext};

impl PolarBearApp {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        #[cfg(target_os = "android")]
        let cloned_android_app = options
            .android_app
            .clone()
            .pb_expect("Failed to clone AndroidApp");
        #[cfg(target_os = "android")]
        ApplicationContext::build(&cloned_android_app);

        let shared = Arc::new(Mutex::new(Shared {
            compositor: None,
            ctx: None,
            logs: VecDeque::new(),
        }));

        let app = PolarBearApp {
            shared: Arc::clone(&shared),
        };

        thread::spawn(move || {
            let result = panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let log = |it| {
                    shared.lock().unwrap().log(it);
                };

                // Step 1. Setup Arch FS if not already installed
                #[cfg(target_os = "android")]
                scaffold::scaffold(&cloned_android_app, log);

                // Step 2. Install dependencies if not already installed
                arch_run_with_log("uname -a", log);
                loop {
                    let installed = arch_run(&"pacman -Qg plasma")
                        .wait()
                        .pb_expect("pacman -Qg plasma failed")
                        .success();
                    if installed {
                        match PolarBearCompositor::build(&cloned_android_app) {
                            Ok(compositor) => {
                                {
                                    shared.lock().unwrap().compositor.replace(compositor);
                                }

                                log(log_format(
                                    "POLAR BEAR COMPOSITOR STARTED",
                                    "Polar Bear Compositor started successfully",
                                ));

                                arch_run_with_log(
                                    &format!(
                                            "HOME=/root XDG_RUNTIME_DIR={} WAYLAND_DISPLAY={} WAYLAND_DEBUG=client weston --fullscreen 2>&1",
                                            // "HOME=/root XDG_RUNTIME_DIR={} WAYLAND_DISPLAY={} WAYLAND_DEBUG=client dbus-run-session startplasma-wayland 2>&1",
                                            config::XDG_RUNTIME_DIR,
                                            config::WAYLAND_SOCKET_NAME),
                                    log,
                                );
                            }
                            Err(e) => {
                                log(log_format(
                                    "POLAR BEAR COMPOSITOR RUNTIME ERROR",
                                    &format!("{}", e),
                                ));
                            }
                        }
                        break;
                    } else {
                        arch_run("rm /var/lib/pacman/db.lck");
                        arch_run_with_log("pacman -Syu plasma weston --noconfirm", log);
                    }
                }
            }));
            if let Err(e) = result {
                let error_msg = e
                    .downcast_ref::<&str>()
                    .map(|s| *s)
                    .or_else(|| e.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("Unknown error");

                shared.lock().unwrap().log(log_format(
                    "POLAR BEAR COMPOSITOR RUNTIME ERROR",
                    &format!("{}", error_msg),
                ));
            }
        });
        eframe::run_native("Polar Bear", options, Box::new(|_cc| Ok(Box::new(app))))
    }
}

impl eframe::App for PolarBearApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if cfg!(debug_assertions) {
            ctx.set_debug_on_hover(true);
        }

        if cfg!(debug_assertions) {
            egui::Window::new("Inspection")
                .resizable(true)
                .default_width(320.0)
                .show(ctx, |ui| {
                    ctx.inspection_ui(ui);
                });
        }

        egui::Window::new("Logs")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.label(
                            self.shared
                                .lock()
                                .unwrap()
                                .logs
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                    });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Vec2::new(0.0, 0.0)))
            .show(ctx, |ui| {
                let renderer = PolarBearRenderer {
                    painter: ui.painter().clone(),
                };
                if let Some(compositor) = self.shared.lock().unwrap().compositor.as_mut() {
                    match compositor.draw(renderer, ui.available_size()) {
                        Ok(_) => {}
                        Err(e) => {
                            self.shared.lock().unwrap().log(log_format(
                                "POLAR BEAR COMPOSITOR DRAW ERROR",
                                &format!("{}", e),
                            ));
                        }
                    }
                };
            });

        self.shared.lock().unwrap().ctx = Some(ctx.clone());
    }
}
