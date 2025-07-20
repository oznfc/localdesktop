use super::logging::PolarBearExpectation;
use serde::{Deserialize, Serialize};
use std::fs;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(not(test))]
pub const ARCH_FS_ROOT: &str = "/data/data/app.polarbear/files/arch";
#[cfg(test)]
pub const ARCH_FS_ROOT: &str = "/data/local/tmp/arch";

pub const ARCH_FS_ARCHIVE: &str = "https://github.com/termux/proot-distro/releases/download/v4.22.1/archlinux-aarch64-pd-v4.22.1.tar.xz";

pub const WAYLAND_SOCKET_NAME: &str = "wayland-0";

pub const MAX_PANEL_LOG_ENTRIES: usize = 100;

pub const SENTRY_DSN: &str = "https://38b0318da81ccc308c2c75686371ddda@o4509548388417536.ingest.de.sentry.io/4509548392480848";

pub const CONFIG_FILE: &str = "/etc/localdesktop/localdesktop.toml";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct LocalConfig {
    pub user: UserConfig,
    pub command: CommandConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserConfig {
    pub username: String,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            username: "root".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandConfig {
    pub check: String,
    pub install: String,
    pub launch: String,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            check: "pacman -Q xorg-xwayland && pacman -Qg xfce4 && pacman -Q onboard".to_string(),
            install:
                "stdbuf -oL pacman -Syu xorg-xwayland xfce4 onboard --noconfirm --noprogressbar".to_string(),
            launch: "XDG_RUNTIME_DIR=/tmp Xwayland -hidpi :1 2>&1 & while [ ! -e /tmp/.X11-unix/X1 ]; do sleep 0.1; done; XDG_SESSION_TYPE=x11 DISPLAY=:1 dbus-launch startxfce4 2>&1"
                .to_string(),
        }
    }
}
pub fn parse_config() -> LocalConfig {
    let config_path = format!("{}{}", ARCH_FS_ROOT, CONFIG_FILE);

    fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str::<LocalConfig>(&content).ok())
        .unwrap_or_default()
}

pub fn save_config(config: LocalConfig) {
    // Create config directory if it doesn't exist
    let config_dir = format!("{}/etc/localdesktop", ARCH_FS_ROOT);
    fs::create_dir_all(&config_dir).pb_expect("Failed to create config directory");

    // Create and write config file
    let config_path = format!("{}{}", ARCH_FS_ROOT, CONFIG_FILE);
    let config_str = toml::to_string(&config).pb_expect("Failed to serialize config");
    fs::write(&config_path, config_str).pb_expect("Failed to write config file");
}
