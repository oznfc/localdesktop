pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(not(test))]
pub const ARCH_FS_ROOT: &str = "/data/data/app.polarbear/files/arch";
#[cfg(test)]
pub const ARCH_FS_ROOT: &str = "/data/local/tmp/arch";

pub const XDG_RUNTIME_DIR: &str = "/tmp"; // Main compositor (Weston/KDE), running in **emulated's process** (PRoot), will connect to the socket here

pub const ARCH_FS_ARCHIVE: &str = "archlinux-aarch64-pd-v4.6.0.tar.xz";

pub const WAYLAND_SOCKET_NAME: &str = "wayland-pb";

pub const MAX_PANEL_LOG_ENTRIES: usize = 100;
