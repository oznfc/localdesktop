use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::{panic, thread};

use smithay::backend::input::{InputBackend, InputEvent, KeyboardKeyEvent};
use smithay::input::keyboard::FilterResult;
use smithay::utils::{Clock, Monotonic, Physical, Size};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Touch, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::android::activity::AndroidApp;
use winit::window::{Window, WindowId};

use crate::arch::scaffold::scaffold;
use crate::arch::setup::{setup, SetupOptions};
use crate::utils::config;
use crate::utils::logging::{log_format, PolarBearExpectation};

use super::compositor::PolarBearCompositor;
use super::input::{
    RelativePosition, WinitInput, WinitKeyboardInputEvent, WinitMouseInputEvent,
    WinitMouseMovedEvent, WinitMouseWheelEvent, WinitTouchCancelledEvent, WinitTouchEndedEvent,
    WinitTouchMovedEvent, WinitTouchStartedEvent,
};
use super::keymap::physicalkey_to_scancode;

pub struct ImmutableAppProperties {
    pub compositor: Option<PolarBearCompositor>,
    logs: VecDeque<String>,
}

impl ImmutableAppProperties {
    pub fn log(&mut self, content: String) {
        self.logs.push_back(content);
        // Ensure the logs size stays at most 20
        if self.logs.len() > config::MAX_PANEL_LOG_ENTRIES {
            self.logs.pop_front();
        }
    }
}

#[derive(Clone)]
pub struct CloneableAppProperties {
    pub inner: Arc<Mutex<ImmutableAppProperties>>,
    pub android_app: AndroidApp,
}

pub struct PolarBearApp {
    pub cloneable: CloneableAppProperties,

    window: Option<Window>,
    clock: Clock<Monotonic>,
    key_counter: u32,
    is_x11: bool,
    scale_factor: f64,
}

impl PolarBearApp {
    pub fn build(android_app: AndroidApp) -> Self {
        let inner = Arc::new(Mutex::new(ImmutableAppProperties {
            compositor: None,
            logs: VecDeque::new(),
        }));

        let cloneable = CloneableAppProperties {
            inner: inner.clone(),
            android_app,
        };

        let cloned_app = cloneable.clone();
        thread::spawn(move || {
            let result = panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Step 1. Setup Arch FS if not already installed
                scaffold(&cloned_app);

                // Step 2. Install dependencies if not already installed
                // let launch_command =
                //     "XDG_SESSION_DESKTOP=KDE XDG_CURRENT_DESKTOP=KDE /usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland".to_string();
                // let launch_command = "weston --fullscreen --scale=2".to_string();
                // let launch_command = "Hyprland".to_string();
                let launch_command =
                    "XDG_SESSION_DESKTOP=LXQT XDG_CURRENT_DESKTOP=LXQT dbus-launch startlxqt"
                        .to_string();

                setup(
                    &cloned_app,
                    SetupOptions {
                        username: "teddy".to_string(), // todo!("Ask the user what username they want to use, and load the answer from somewhere")
                        checking_command: "pacman -Qg lxqt && pacman -Q breeze-icons".to_string(),
                        install_packages: "lxqt breeze-icons".to_string(),
                        launch_command,
                    },
                );
            }));
            if let Err(e) = result {
                let error_msg = e
                    .downcast_ref::<&str>()
                    .map(|s| *s)
                    .or_else(|| e.downcast_ref::<String>().map(|s| s.as_str()))
                    .unwrap_or("Unknown error");

                inner.lock().unwrap().log(log_format(
                    "POLAR BEAR COMPOSITOR RUNTIME ERROR",
                    &format!("{}", error_msg),
                ));
            }
        });

        Self {
            cloneable,

            window: None,
            clock: Clock::new(),
            key_counter: 0,
            is_x11: false,
            scale_factor: 1.0,
        }
    }

    fn timestamp(&self) -> u64 {
        self.clock.now().as_millis() as u64
    }
}

impl ApplicationHandler for PolarBearApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Map raw events to our own events
        let (event) = match event {
            WindowEvent::Resized(size) => {
                let (w, h): (i32, i32) = size.into();

                (CentralizedEvent::Resized {
                    size: (w, h).into(),
                    scale_factor: self.scale_factor,
                })
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: new_scale_factor,
                ..
            } => {
                self.scale_factor = new_scale_factor;
                let (w, h): (i32, i32) = self.window.as_ref().unwrap().inner_size().into();
                (CentralizedEvent::Resized {
                    size: (w, h).into(),
                    scale_factor: self.scale_factor,
                })
            }
            WindowEvent::RedrawRequested => (CentralizedEvent::Redraw),
            WindowEvent::CloseRequested => (CentralizedEvent::CloseRequested),
            WindowEvent::Focused(focused) => (CentralizedEvent::Focus(focused)),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } if !is_synthetic && !event.repeat => {
                match event.state {
                    ElementState::Pressed => self.key_counter += 1,
                    ElementState::Released => {
                        self.key_counter = self.key_counter.saturating_sub(1);
                    }
                };

                let scancode = physicalkey_to_scancode(event.physical_key).unwrap_or(0);
                let event = InputEvent::Keyboard {
                    event: WinitKeyboardInputEvent {
                        time: self.timestamp(),
                        key: scancode,
                        count: self.key_counter,
                        state: event.state,
                    },
                };
                (CentralizedEvent::Input(event))
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.window.as_ref().unwrap().inner_size();
                let x = position.x / size.width as f64;
                let y = position.y / size.height as f64;
                let event = InputEvent::PointerMotionAbsolute {
                    event: WinitMouseMovedEvent {
                        time: self.timestamp(),
                        position: RelativePosition::new(x, y),
                        global_position: position,
                    },
                };
                (CentralizedEvent::Input(event))
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let event = InputEvent::PointerAxis {
                    event: WinitMouseWheelEvent {
                        time: self.timestamp(),
                        delta,
                    },
                };
                (CentralizedEvent::Input(event))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let event = InputEvent::PointerButton {
                    event: WinitMouseInputEvent {
                        time: self.timestamp(),
                        button,
                        state,
                        is_x11: self.is_x11,
                    },
                };
                (CentralizedEvent::Input(event))
            }
            WindowEvent::Touch(winit::event::Touch {
                phase: TouchPhase::Started,
                location,
                id,
                ..
            }) => {
                let size = self.window.as_ref().unwrap().inner_size();
                let x = location.x / size.width as f64;
                let y = location.y / size.width as f64;
                let event = InputEvent::TouchDown {
                    event: WinitTouchStartedEvent {
                        time: self.timestamp(),
                        global_position: location,
                        position: RelativePosition::new(x, y),
                        id,
                    },
                };

                (CentralizedEvent::Input(event))
            }
            WindowEvent::Touch(Touch {
                phase: TouchPhase::Moved,
                location,
                id,
                ..
            }) => {
                let size = self.window.as_ref().unwrap().inner_size();
                let x = location.x / size.width as f64;
                let y = location.y / size.width as f64;
                let event = InputEvent::TouchMotion {
                    event: WinitTouchMovedEvent {
                        time: self.timestamp(),
                        position: RelativePosition::new(x, y),
                        global_position: location,
                        id,
                    },
                };

                (CentralizedEvent::Input(event))
            }

            WindowEvent::Touch(Touch {
                phase: TouchPhase::Ended,
                location,
                id,
                ..
            }) => {
                let size = self.window.as_ref().unwrap().inner_size();
                let x = location.x / size.width as f64;
                let y = location.y / size.width as f64;
                let event = InputEvent::TouchMotion {
                    event: WinitTouchMovedEvent {
                        time: self.timestamp(),
                        position: RelativePosition::new(x, y),
                        global_position: location,
                        id,
                    },
                };
                (CentralizedEvent::Input(event));

                let event = InputEvent::TouchUp {
                    event: WinitTouchEndedEvent {
                        time: self.timestamp(),
                        id,
                    },
                };

                (CentralizedEvent::Input(event))
            }

            WindowEvent::Touch(Touch {
                phase: TouchPhase::Cancelled,
                id,
                ..
            }) => {
                let event = InputEvent::TouchCancel {
                    event: WinitTouchCancelledEvent {
                        time: self.timestamp(),
                        id,
                    },
                };
                (CentralizedEvent::Input(event))
            }
            _ => panic!("Unhandled event: {:?}", event),
        };

        // Handle the centralized events
        match event {
            CentralizedEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            CentralizedEvent::Redraw => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                // Draw.

                // Queue a RedrawRequested event.
                //
                // You only need to call this if you've determined that you need to redraw in
                // applications which do not always need to. Applications that redraw continuously
                // can render here instead.
                self.window.as_ref().unwrap().request_redraw();
            }
            CentralizedEvent::Input(event) => match event {
                InputEvent::Keyboard { event } => {
                    let mut lock = self.cloneable.inner.lock().unwrap();
                    let compositor = lock.compositor.as_mut().unwrap();
                    let state = &mut compositor.state;
                    compositor.keyboard.input::<(), _>(
                        state,
                        event.key_code(),
                        event.state(),
                        0.into(),
                        0,
                        |_, _, _| {
                            //
                            FilterResult::Forward
                        },
                    );
                }
                InputEvent::PointerMotionAbsolute { .. } => {
                    let mut lock = self.cloneable.inner.lock().unwrap();
                    let compositor = lock.compositor.as_mut().unwrap();
                    let state = &mut compositor.state;
                    if let Some(surface) = state
                        .xdg_shell_state
                        .toplevel_surfaces()
                        .iter()
                        .next()
                        .cloned()
                    {
                        let surface = surface.wl_surface().clone();
                        compositor
                            .keyboard
                            .set_focus(state, Some(surface), 0.into());
                    };
                }
                _ => {}
            },
            _ => (),
        }
    }
}

/// Specific events generated by Winit
#[derive(Debug)]
pub enum CentralizedEvent {
    /// The window has been resized
    Resized {
        /// The new physical size (in pixels)
        size: Size<i32, Physical>,
        /// The new scale factor
        scale_factor: f64,
    },

    /// The focus state of the window changed
    Focus(bool),

    /// An input event occurred.
    Input(InputEvent<WinitInput>),

    /// The user requested to close the window.
    CloseRequested,

    /// A redraw was requested
    Redraw,
}
