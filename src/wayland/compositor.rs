use std::{error::Error, path::PathBuf};

use wayland_server::{Client, Display, ListeningSocket};

use crate::utils::config;

pub struct PolarBearCompositor {}

impl PolarBearCompositor {
    pub fn run(&self) -> Result<(), Box<dyn Error>> {
        let mut display: Display<PolarBearCompositor> = Display::new()?;
        let dh = display.handle();

        #[cfg(target_os = "android")]
        let arch_fs: PathBuf = config::ARCH_FS_ROOT.into();
        #[cfg(not(target_os = "android"))]
        let arch_fs: PathBuf = expanduser::expanduser(config::ARCH_FS_ROOT)?.into();

        let socket_path = arch_fs.join("tmp").join(config::WAYLAND_SOCKET_NAME);
        let listener = ListeningSocket::bind_absolute(socket_path)?;
        let mut clients: Vec<Client> = Vec::new();
        let start_time = std::time::Instant::now();

        std::env::set_var("WAYLAND_DISPLAY", config::WAYLAND_SOCKET_NAME);
        return Ok(());
    }
}
