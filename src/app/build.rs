use crate::proot::setup::setup;
use crate::utils::logging::PolarBearExpectation;
use crate::wayland::compositor::Compositor;
use crate::wayland::winit_backend::WinitGraphicsBackend;
use serde_json::json;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::{Clock, Monotonic};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use websocket::sync::Server;
use websocket::OwnedMessage;
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
    pub socket_port: u16,
    pub progress: Arc<Mutex<u16>>, // 0-100
}

impl WebviewBackend {
    /// Start accepting connections and listening for messages
    pub fn build(receiver: Receiver<String>, progress: Arc<Mutex<u16>>) -> Self {
        let socket = Server::bind("127.0.0.1:0").pb_expect("Failed to bind socket");
        let socket_port = socket.local_addr().unwrap().port();

        let active_client = Arc::new(Mutex::new(None));
        let receiver = Arc::new(Mutex::new(receiver));

        let active_client_clone = active_client.clone();
        let progress_clone = progress.clone();
        thread::spawn(move || {
            for request in socket.filter_map(Result::ok) {
                let mut active_client = active_client_clone.lock().unwrap();

                // Reject new connections if there is already an active client
                if active_client.is_some() {
                    println!("Rejecting new connection: already an active client");
                    request.reject().unwrap();
                    continue;
                }

                // Accept the new client
                if !request.protocols().contains(&"rust-websocket".to_string()) {
                    request.reject().unwrap();
                    continue;
                }

                let client = request.use_protocol("rust-websocket").accept().unwrap();
                let ip = client.peer_addr().unwrap();
                println!("Connection from {}", ip);

                // Store the new client
                *active_client = Some(client); // Store the writer part of the connection

                // Spawn a thread to handle messages for this client
                let active_client_clone = active_client_clone.clone();
                let receiver_clone = receiver.clone();
                let progress_clone = progress_clone.clone();
                thread::spawn(move || {
                    for message in receiver_clone.lock().unwrap().iter() {
                        let progress = *progress_clone.lock().unwrap();
                        let json_message = json!({
                            "progress": progress,
                            "message": message
                        });

                        let message = OwnedMessage::Text(json_message.to_string());
                        let mut active_client = active_client_clone.lock().unwrap();

                        if let Some(writer) = active_client.as_mut() {
                            if writer.send_message(&message).is_err() {
                                // If sending fails, disconnect the client
                                println!("Client disconnected");
                                *active_client = None;
                                break;
                            }
                        }
                    }
                });
            }
        });

        Self {
            socket_port,
            progress,
        }
    }
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
            backend: setup(android_app.clone()),
            frontend: PolarBearFrontend { android_app },
        }
    }
}
