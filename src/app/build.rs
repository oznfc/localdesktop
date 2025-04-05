use crate::proot::setup::{setup, SetupOptions};
use crate::wayland::compositor::Compositor;
use crate::wayland::winit_backend::WinitGraphicsBackend;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Clock, Monotonic};
use std::net::TcpListener;
use websocket::server::{NoTlsAcceptor, WsServer};
use winit::platform::android::activity::AndroidApp;

pub struct PolarBearApp {
    pub frontend: PolarBearFrontend,
    pub backend: PolarBearBackend,
}

pub struct PolarBearFrontend {
    pub android_app: AndroidApp,
}

pub enum PolarBearBackend {
    /// Use a webview to report setup progress to the user
    /// The setup progress should only be done once, when the user first installed the app
    WebView(WebviewBackend),

    /// Use a wayland compositor to render Linux GUI applications back to the Android Native Activity
    Wayland(WaylandBackend),
}

pub struct WebviewBackend {
    pub socket: WsServer<NoTlsAcceptor, TcpListener>,
    pub last_message: String,
}

pub struct WaylandBackend {
    pub compositor: Compositor,
    pub graphic_renderer: Option<WinitGraphicsBackend<GlesRenderer>>,
    pub clock: Clock<Monotonic>,
    pub key_counter: u32,
    pub scale_factor: f64,
}

impl PolarBearApp {
    pub fn build(android_app: AndroidApp) -> Self {
        Self {
            backend: setup(SetupOptions {
                username: "teddy".to_string(), // todo!("Ask the user what username they want to use, and load the answer from somewhere")
                checking_command:
                    "pacman -Q xorg-xwayland && pacman -Qg xfce4 && pacman -Q onboard".to_string(), // TODO: Break these steps down into independent checks and installs
                install_packages: "xorg-xwayland xfce4 onboard".to_string(),
                android_app: android_app.clone(),
            }),
            frontend: PolarBearFrontend { android_app },
        }
    }
}
