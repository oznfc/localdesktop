use std::{
    error::Error,
    os::unix::io::OwnedFd,
    sync::{Arc, Mutex},
    time::Instant, // Added import
};

use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_data_device, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    input::{self, keyboard::KeyboardHandle, touch::TouchHandle, Seat, SeatHandler, SeatState},
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{protocol::wl_seat, Display},
    },
    utils::{Serial, Size, Transform},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            with_surface_tree_downward, CompositorClientState, CompositorHandler, CompositorState,
            SurfaceAttributes, TraversalAction,
        },
        output::OutputHandler,
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
            },
            SelectionHandler,
        },
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
    },
};
use wayland_server::{
    backend::{ClientData, ClientId, DisconnectReason},
    protocol::{
        wl_buffer,
        wl_surface::{self, WlSurface},
    },
    Client, ListeningSocket,
};
use winit::platform::android::activity::AndroidApp;

use crate::utils::{config, logging::PolarBearExpectation, wayland::bind_socket};

pub struct PolarBearCompositor {
    pub state: State,
    display: Display<State>,
    listener: ListeningSocket,
    clients: Arc<Mutex<Vec<Client>>>,
    pub start_time: Instant,
    seat: Seat<State>,
    pub keyboard: KeyboardHandle<State>,
    pub touch: TouchHandle<State>,
    output: Output,
}

pub struct State {
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub data_device_state: DataDeviceState,
    pub seat_state: SeatState<Self>,
    size: (i32, i32),
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.size.replace(Size::from(self.size));
            state.states.set(xdg_toplevel::State::Activated);
        });
        surface.send_configure();
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Handle popup creation here
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // Handle popup grab here
    }

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
        // Handle popup reposition here
    }
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {}
impl ServerDndGrabHandler for State {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: input::pointer::CursorImageStatus) {}
}

pub fn send_frames_surface_tree(surface: &wl_surface::WlSurface, time: u32) {
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

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {
        println!("initialized");
    }

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        println!("disconnected");
    }
}

impl OutputHandler for State {}

// Macros used to delegate protocol handling to types in the app state.
delegate_xdg_shell!(State);
delegate_compositor!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_data_device!(State);
delegate_output!(State);

impl PolarBearCompositor {
    pub fn build(app: &AndroidApp) -> Result<PolarBearCompositor, Box<dyn Error>> {
        let display = Display::new()?;
        let dh = display.handle();

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "Polar Bear");

        let listener = bind_socket()?;
        let clients = Arc::new(Mutex::new(Vec::new()));

        let start_time = Instant::now();

        // Key repeat rate and delay are in milliseconds: https://wayland-book.com/seat/keyboard.html
        let keyboard = seat.add_keyboard(Default::default(), 1000, 200).unwrap();
        let touch = seat.add_touch();

        let native_window = app.native_window().pb_expect("Failed to get ANativeWindow");
        let display_width = native_window.width();
        let display_height = native_window.height();
        let size = (display_width, display_height);
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
            Some(Scale::Integer(1)), // global screen scaling factor
            Some((0, 0).into()),     // output position
        );
        // set the preferred mode
        output.set_preferred(Mode {
            size: size.into(),
            refresh: 60000,
        });

        let state = State {
            compositor_state: CompositorState::new::<State>(&dh),
            xdg_shell_state: XdgShellState::new::<State>(&dh),
            shm_state: ShmState::new::<State>(&dh, vec![]),
            data_device_state: DataDeviceState::new::<State>(&dh),
            seat_state,
            size,
        };

        Ok(PolarBearCompositor {
            state,
            listener,
            clients,
            start_time,
            display,
            seat,
            keyboard,
            touch,
            output,
        })
    }
}
