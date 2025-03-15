use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::{panic, thread};

use smithay::backend::input::{
    AbsolutePositionEvent, InputEvent, KeyboardKeyEvent, TouchEvent, TouchSlot,
};
use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::utils::draw_render_elements;
use smithay::backend::renderer::{Color32F, Frame, Renderer};
use smithay::input::keyboard::FilterResult;
use smithay::input::touch::{DownEvent, UpEvent};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{
    Clock, Monotonic, Physical, Point, Rectangle, Size, Transform, SERIAL_COUNTER,
};
use smithay::wayland::compositor::{
    with_surface_tree_downward, SurfaceAttributes, TraversalAction,
};
use wayland_server::protocol::wl_surface::WlSurface;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Touch, TouchPhase, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::platform::android::activity::AndroidApp;
use winit::window::WindowId;

use crate::app::compositor::ClientState;
use crate::arch::scaffold::scaffold;
use crate::arch::setup::{launch, setup, SetupOptions};
use crate::utils::config;
use crate::utils::logging::PolarBearExpectation;

use super::compositor::{PolarBearCompositor, State};
use super::input::{
    RelativePosition, WinitInput, WinitKeyboardInputEvent, WinitMouseInputEvent,
    WinitMouseMovedEvent, WinitMouseWheelEvent, WinitTouchCancelledEvent, WinitTouchEndedEvent,
    WinitTouchMovedEvent, WinitTouchStartedEvent,
};
use super::keymap::physicalkey_to_scancode;
use super::winit::{bind, WinitGraphicsBackend};

pub struct PolarBearLogging {
    logs: VecDeque<String>,
}

impl PolarBearLogging {
    pub fn log(&mut self, content: String) {
        println!("🐻‍❄️ {}", content);
        self.logs.push_back(content);
        // Ensure the logs size stays at most 20
        if self.logs.len() > config::MAX_PANEL_LOG_ENTRIES {
            self.logs.pop_front();
        }
    }
}

pub struct PolarBearApp {
    pub logging: Arc<Mutex<PolarBearLogging>>,
    pub compositor: PolarBearCompositor,
    pub backend: Option<WinitGraphicsBackend<GlesRenderer>>,
    clock: Clock<Monotonic>,
    key_counter: u32,
    scale_factor: f64,
}

impl PolarBearApp {
    pub fn build(android_app: AndroidApp) -> Self {
        let logging = Arc::new(Mutex::new(PolarBearLogging {
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
            checking_command: "pacman -Qg plasma".to_string(),
            install_packages: "plasma".to_string(),
            log: Box::new(log.clone()),
            android_app,
        });

        Self {
            logging,
            compositor,
            backend: None,
            clock: Clock::new(),
            key_counter: 0,
            scale_factor: 1.0,
        }
    }

    fn timestamp(&self) -> u64 {
        self.clock.now().as_millis() as u64
    }
}

impl ApplicationHandler for PolarBearApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let backend = bind(&event_loop);
        let window_size = backend.window_size();
        let scale_factor = backend.scale_factor();
        let size = (window_size.w, window_size.h);
        self.backend = Some(backend);
        self.compositor.state.size = size.into();

        // Create the Output with given name and physical properties.
        let output = Output::new(
            "Polar Bear Wayland Compositor".into(), // the name of this output,
            PhysicalProperties {
                size: size.into(),                 // dimensions (width, height) in mm
                subpixel: Subpixel::HorizontalRgb, // subpixel information
                make: "Polar Bear".into(),         // make of the monitor
                model: config::VERSION.into(),     // model of the monitor
            },
        );

        let dh = self.compositor.display.handle();
        // create a global, if you want to advertise it to clients
        let _global = output.create_global::<State>(
            &dh, // the display
        ); // you can drop the global, if you never intend to destroy it.
           // Now you can configure it
        output.change_current_state(
            Some(Mode {
                size: size.into(),
                refresh: 60000,
            }), // the resolution mode,
            Some(Transform::Normal), // global screen transformation
            Some(Scale::Fractional(scale_factor)), // global screen scaling factor
            Some((0, 0).into()),     // output position
        );
        // set the preferred mode
        output.set_preferred(Mode {
            size: size.into(),
            refresh: 60000,
        });

        thread::spawn(move || {
            let launch_command =
                "XDG_SESSION_DESKTOP=KDE XDG_CURRENT_DESKTOP=KDE /usr/lib/plasma-dbus-run-session-if-needed /usr/bin/startplasma-wayland".to_string();
            // let launch_command = format!("weston --fullscreen --scale={}", scale_factor);
            // let launch_command =
            //     "XDG_SESSION_DESKTOP=LXQT XDG_CURRENT_DESKTOP=LXQT dbus-launch startlxqt"
            //         .to_string();
            launch(launch_command);
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Map raw events to our own events
        let event = match event {
            WindowEvent::Resized(size) => {
                let (w, h): (i32, i32) = size.into();

                CentralizedEvent::Resized {
                    size: (w, h).into(),
                    scale_factor: self.scale_factor,
                }
            }
            WindowEvent::ScaleFactorChanged {
                scale_factor: new_scale_factor,
                ..
            } => {
                self.scale_factor = new_scale_factor;
                let (w, h): (i32, i32) =
                    self.backend.as_ref().unwrap().window().inner_size().into();
                CentralizedEvent::Resized {
                    size: (w, h).into(),
                    scale_factor: self.scale_factor,
                }
            }
            WindowEvent::RedrawRequested => CentralizedEvent::Redraw,
            WindowEvent::CloseRequested => CentralizedEvent::CloseRequested,
            WindowEvent::Focused(focused) => CentralizedEvent::Focus(focused),
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
                CentralizedEvent::Input(event)
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.backend.as_ref().unwrap().window().inner_size();
                let x = position.x / size.width as f64;
                let y = position.y / size.height as f64;
                let event = InputEvent::PointerMotionAbsolute {
                    event: WinitMouseMovedEvent {
                        time: self.timestamp(),
                        position: RelativePosition::new(x, y),
                        global_position: position,
                    },
                };
                CentralizedEvent::Input(event)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let event = InputEvent::PointerAxis {
                    event: WinitMouseWheelEvent {
                        time: self.timestamp(),
                        delta,
                    },
                };
                CentralizedEvent::Input(event)
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let event = InputEvent::PointerButton {
                    event: WinitMouseInputEvent {
                        time: self.timestamp(),
                        button,
                        state,
                        is_x11: false,
                    },
                };
                CentralizedEvent::Input(event)
            }
            WindowEvent::Touch(winit::event::Touch {
                phase: TouchPhase::Started,
                location,
                id,
                ..
            }) => {
                let size = self.backend.as_ref().unwrap().window().inner_size();
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

                CentralizedEvent::Input(event)
            }
            WindowEvent::Touch(Touch {
                phase: TouchPhase::Moved,
                location,
                id,
                ..
            }) => {
                let size = self.backend.as_ref().unwrap().window().inner_size();
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

                CentralizedEvent::Input(event)
            }

            WindowEvent::Touch(Touch {
                phase: TouchPhase::Ended,
                location,
                id,
                ..
            }) => {
                let size = self.backend.as_ref().unwrap().window().inner_size();
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

                CentralizedEvent::Input(event)
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
                CentralizedEvent::Input(event)
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
                if let Some(mut backend) = self.backend.as_mut() {
                    let size = backend.window_size();
                    let damage = Rectangle::from_size(size);
                    {
                        let (renderer, mut framebuffer) = backend.bind().unwrap();

                        let compositor = &mut self.compositor;

                        let elements = compositor
                            .state
                            .xdg_shell_state
                            .toplevel_surfaces()
                            .iter()
                            .flat_map(|surface| {
                                render_elements_from_surface_tree(
                                    renderer,
                                    surface.wl_surface(),
                                    (0, 0),
                                    1.0,
                                    1.0,
                                    Kind::Unspecified,
                                )
                            })
                            .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

                        let mut frame = renderer
                            .render(&mut framebuffer, size, Transform::Flipped180)
                            .unwrap();
                        frame
                            .clear(Color32F::new(0.1, 0.0, 0.0, 1.0), &[damage])
                            .unwrap();
                        draw_render_elements(&mut frame, 1.0, &elements, &[damage]).unwrap();
                        // We rely on the nested compositor to do the sync for us
                        let _ = frame.finish().unwrap();

                        for surface in compositor.state.xdg_shell_state.toplevel_surfaces() {
                            send_frames_surface_tree(
                                surface.wl_surface(),
                                compositor.start_time.elapsed().as_millis() as u32,
                            );
                        }

                        if let Some(stream) = compositor
                            .listener
                            .accept()
                            .pb_expect("Failed to accept listener")
                        {
                            println!("Got a client: {:?}", stream);

                            let client = compositor
                                .display
                                .handle()
                                .insert_client(stream, Arc::new(ClientState::default()))
                                .unwrap();
                            compositor.clients.push(client);
                        }

                        compositor
                            .display
                            .dispatch_clients(&mut compositor.state)
                            .pb_expect("Failed to dispatch clients");
                        compositor
                            .display
                            .flush_clients()
                            .pb_expect("Failed to flush clients");
                    }

                    // It is important that all events on the display have been dispatched and flushed to clients before
                    // swapping buffers because this operation may block.
                    backend.submit(Some(&[damage])).unwrap();
                }

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
                self.backend.as_ref().unwrap().window().request_redraw();
            }
            CentralizedEvent::Input(event) => match event {
                InputEvent::Keyboard { event } => {
                    let compositor = &mut self.compositor;
                    let state = &mut compositor.state;
                    let serial = SERIAL_COUNTER.next_serial();
                    let time = compositor.start_time.elapsed().as_millis() as u32;
                    compositor.keyboard.input::<(), _>(
                        state,
                        event.key_code(),
                        event.state(),
                        serial,
                        time,
                        |_, _, _| {
                            //
                            FilterResult::Forward
                        },
                    );
                }
                InputEvent::TouchDown { event } => {
                    let compositor = &mut self.compositor;
                    let state = &mut compositor.state;
                    if let Some(surface) = state
                        .xdg_shell_state
                        .toplevel_surfaces()
                        .iter()
                        .next()
                        .cloned()
                    {
                        compositor.keyboard.set_focus(
                            state,
                            Some(surface.wl_surface().clone()),
                            0.into(),
                        );
                        let serial = SERIAL_COUNTER.next_serial();
                        let time = compositor.start_time.elapsed().as_millis() as u32;
                        compositor.touch.down(
                            state,
                            Some((surface.wl_surface().clone(), (0f64, 0f64).into())),
                            &DownEvent {
                                slot: event.slot(),
                                location: (event.x(), event.y()).into(),
                                serial,
                                time,
                            },
                        );
                    };
                }
                InputEvent::TouchUp { event } => {
                    let compositor = &mut self.compositor;
                    let state = &mut compositor.state;
                    if let Some(surface) = state
                        .xdg_shell_state
                        .toplevel_surfaces()
                        .iter()
                        .next()
                        .cloned()
                    {
                        let serial = SERIAL_COUNTER.next_serial();
                        let time = compositor.start_time.elapsed().as_millis() as u32;
                        compositor.touch.up(
                            state,
                            &UpEvent {
                                slot: event.slot(),
                                serial,
                                time,
                            },
                        );
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

pub fn send_frames_surface_tree(surface: &WlSurface, time: u32) {
    with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_surf, states, &()| {
            // the surface may not have any user_data if it is a subsurface and has not
            // yet been commited
            for callback in states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .frame_callbacks
                .drain(..)
            {
                callback.done(time);
            }
        },
        |_, _, &()| true,
    );
}
