#![cfg(target_os = "android")]

pub mod android_main;
pub mod app {
    pub mod compositor;
    pub mod renderer;
    pub mod ui;
}
pub mod arch {
    pub mod run;

    pub mod scaffold; // No need for an additional cfg check since the whole crate is Android-only
}
pub mod utils {
    pub mod config;
    pub mod logging;
    pub mod wayland;

    pub mod application_context;
}
