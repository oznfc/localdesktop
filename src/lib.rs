#![cfg(target_os = "android")]

pub mod android_main;
pub mod app {
    pub mod compositor;
    pub mod input;
    pub mod keymap;
    pub mod polar_bear;
    pub mod winit;
}
pub mod arch {
    pub mod process;
    pub mod scaffold;
    pub mod setup;
}
pub mod utils {
    pub mod application_context;
    pub mod config;
    pub mod logging;
    pub mod wayland;
}
