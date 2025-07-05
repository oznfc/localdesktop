use crate::app::backend::wayland::WaylandBackend;
use crate::app::backend::webview::WebviewBackend;
use crate::proot::setup::setup;
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

impl PolarBearApp {
    pub fn build(android_app: AndroidApp) -> Self {
        Self {
            backend: setup(android_app.clone()),
            frontend: PolarBearFrontend { android_app },
        }
    }
}
