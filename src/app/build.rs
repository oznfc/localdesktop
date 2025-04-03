use crate::proot::scaffold::scaffold;
use crate::proot::setup::{setup, SetupOptions};
use crate::utils::config;
use crate::wayland::compositor::Compositor;
use crate::wayland::winit_backend::WinitGraphicsBackend;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Clock, Monotonic};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use winit::platform::android::activity::AndroidApp;

pub struct PolarBearApp {
    pub frontend: PolarBearFrontend,
    pub backend: PolarBearBackend,
    pub data: PolarBearData,
}

pub struct PolarBearFrontend {
    pub android_app: AndroidApp,
    pub log: Arc<Mutex<PolarBearLog>>,
}

pub struct PolarBearBackend {
    pub compositor: Compositor,
    pub graphic_renderer: Option<WinitGraphicsBackend<GlesRenderer>>,
}

pub struct PolarBearData {
    pub clock: Clock<Monotonic>,
    pub key_counter: u32,
    pub scale_factor: f64,
}

pub struct PolarBearLog {
    logs: VecDeque<String>,
}

impl PolarBearLog {
    pub fn log(&mut self, content: String) {
        println!("🐻‍❄️ {}", content);
        self.logs.push_back(content);
        // Ensure the logs size stays at most 20
        if self.logs.len() > config::MAX_PANEL_LOG_ENTRIES {
            self.logs.pop_front();
        }
    }
}

impl PolarBearApp {
    pub fn build(android_app: AndroidApp) -> Self {
        let logging: Arc<Mutex<PolarBearLog>> = Arc::new(Mutex::new(PolarBearLog {
            logs: VecDeque::new(),
        }));

        let cloned_logging = logging.clone();
        let log = move |it| {
            cloned_logging.lock().unwrap().log(it);
        };

        // Step 1. Setup Arch FS if not already installed
        scaffold(android_app.clone(), Box::new(log.clone()));

        // Step 2. Install dependencies if not already installed
        let compositor = setup(SetupOptions {
            username: "teddy".to_string(), // todo!("Ask the user what username they want to use, and load the answer from somewhere")
            checking_command: "pacman -Q xorg-xwayland && pacman -Qg xfce4 && pacman -Q onboard"
                .to_string(), // TODO: Break these steps down into independent checks and installs
            install_packages: "xorg-xwayland xfce4 onboard".to_string(),
            log: Box::new(log.clone()),
            android_app: android_app.clone(),
        });

        Self {
            frontend: PolarBearFrontend {
                log: logging,
                android_app,
            },
            backend: PolarBearBackend {
                compositor,
                graphic_renderer: None,
            },
            data: PolarBearData {
                clock: Clock::new(),
                key_counter: 0,
                scale_factor: 1.0,
            },
        }
    }
}
