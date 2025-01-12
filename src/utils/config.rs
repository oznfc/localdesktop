#[cfg(target_os = "android")]
pub const ARCH_FS_ROOT: &str = "/data/data/app.polarbear/files/arch";
#[cfg(target_os = "macos")]
pub const ARCH_FS_ROOT: &str = "~/OrbStack/arch";

#[cfg(target_os = "android")]
pub const TMP_DIR: &str = "/data/data/app.polarbear/files/arch/tmp"; // Polar Bear's compositor, running in **host's process**, will create Unix socket here
#[cfg(target_os = "macos")]
pub const TMP_DIR: &str = "/tmp";

#[cfg(target_os = "android")]
pub const XDG_RUNTIME_DIR: &str = "/tmp"; // Main compositor (Weston/KDE), running in **emulated's process** (PRoot/OrbStack), will connect to the socket here
#[cfg(target_os = "macos")]
pub const XDG_RUNTIME_DIR: &str = "/mnt/mac/tmp";

pub const ARCH_FS_ARCHIVE: &str = "archlinux-aarch64-pd-v4.6.0.tar.xz";

pub const WAYLAND_SOCKET_NAME: &str = "wayland-pb";

pub const MAX_PANEL_LOG_ENTRIES: usize = 100;
