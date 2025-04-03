#![cfg(target_os = "android")]

pub mod android_main;
pub mod app {
    pub mod build;
    pub mod run;
}
pub mod wayland {
    pub mod compositor;
    pub mod input;
    pub mod keymap;
    pub mod winit_backend;
}
pub mod proot {
    pub mod process;
    pub mod scaffold;
    pub mod setup;
}
pub mod utils {
    pub mod application_context;
    pub mod config;
    pub mod fullscreen_immersive;
    pub mod logging;
    pub mod ndk;
    pub mod socket;
}
