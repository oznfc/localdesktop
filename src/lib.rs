pub mod app {
    pub mod compositor;
    pub mod renderer;
    pub mod ui;
}
pub mod arch {
    pub mod run;

    #[cfg(target_os = "android")]
    pub mod scaffold;
}
pub mod utils {
    pub mod config;
    pub mod logging;
    pub mod wayland;

    #[cfg(target_os = "android")]
    pub mod application_context;
}

#[cfg(target_os = "android")]
pub mod android_main;
