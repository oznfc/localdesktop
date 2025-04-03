use std::{error::Error, path::PathBuf};

use wayland_server::ListeningSocket;

use crate::utils::config;

pub fn bind_socket() -> Result<ListeningSocket, Box<dyn Error>> {
    let socket_path =
        PathBuf::from(config::ARCH_FS_ROOT.to_owned() + "/tmp").join(config::WAYLAND_SOCKET_NAME);
    let listener = ListeningSocket::bind_absolute(socket_path)?;
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::FileTypeExt;

    use super::*;

    #[test]
    fn should_bind_socket_successfully() {
        let result = bind_socket();
        // make sure there is a `wayland-pb` socket in /tmp
        assert!(result.is_ok(), "Result is not ok");
        let socket_path = PathBuf::from(config::ARCH_FS_ROOT.to_owned() + "/tmp")
            .join(config::WAYLAND_SOCKET_NAME);
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
