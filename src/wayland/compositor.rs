use std::{error::Error, panic::RefUnwindSafe, path::PathBuf};

use wayland_server::{Client, Display, ListeningSocket};

use crate::utils::config;

#[derive(Debug)]
pub struct PolarBearCompositor {
    listener: ListeningSocket,
    clients: Vec<Client>,
}

impl RefUnwindSafe for PolarBearCompositor {}

impl PolarBearCompositor {
    pub fn build() -> Result<PolarBearCompositor, Box<dyn Error>> {
        let mut display: Display<PolarBearCompositor> = Display::new()?;
        let dh = display.handle();

        let socket_path = PathBuf::from(config::TMP_DIR).join(config::WAYLAND_SOCKET_NAME);
        let listener = ListeningSocket::bind_absolute(socket_path)?;
        let mut clients: Vec<Client> = Vec::new();
        let start_time = std::time::Instant::now();

        std::env::set_var("WAYLAND_DISPLAY", config::WAYLAND_SOCKET_NAME);

        Ok(PolarBearCompositor { listener, clients })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::FileTypeExt;

    use super::*;

    #[test]
    fn compositor_should_build_successfully() {
        let result = PolarBearCompositor::build();
        // make sure there is a `wayland-pb` socket in TMP_DIR
        assert!(result.is_ok(), "Result is not ok");
        let socket_path = PathBuf::from(config::TMP_DIR).join(config::WAYLAND_SOCKET_NAME);
        assert!(socket_path.exists(), "Socket does not exist");
        assert!(
            socket_path
                .metadata()
                .map(|m| m.file_type().is_socket())
                .unwrap_or(false),
            "Socket is not a socket"
        );
    }
}
