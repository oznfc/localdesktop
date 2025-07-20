use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(not(test))]
pub const ARCH_FS_ROOT: &str = "/data/data/app.polarbear/files/arch";
#[cfg(test)]
pub const ARCH_FS_ROOT: &str = "/data/local/tmp/arch";

pub const ARCH_FS_ARCHIVE: &str = "https://github.com/termux/proot-distro/releases/download/v4.22.1/archlinux-aarch64-pd-v4.22.1.tar.xz";

pub const WAYLAND_SOCKET_NAME: &str = "wayland-0";

pub const MAX_PANEL_LOG_ENTRIES: usize = 100;

pub const SENTRY_DSN: &str = "https://38b0318da81ccc308c2c75686371ddda@o4509548388417536.ingest.de.sentry.io/4509548392480848";

/// Make sure the config keys are all lowercase, and config values are single-line. Use \n for multi-line config values if needed
/// If a key exists multiple time, the first entry is applied
/// If a `try_` config exsists multiple time, the last entry is applied
/// But in general, it is **invalid** to have duplicated config keys inside a TOML file
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

/// This function does 2 major tasks:
/// - Read config from `CONFIG_FILE`, and override configs with their `try_*` versions, and return the configs line by line
/// - Write back to the config file, with `try_*` configs commented out
fn process_config_file() -> Vec<String> {
    let full_config_path = format!("{}{}", ARCH_FS_ROOT, CONFIG_FILE);

    let mut write_back_lines: Vec<String> = vec![];
    let mut effective_config: Vec<String> = vec![];

    if let Ok(content) = fs::read_to_string(&full_config_path) {
        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') {
                write_back_lines.push(line.to_string());
                continue;
            }

            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                if key.starts_with("try_") {
                    // Comment out the `try_*` configs
                    write_back_lines.push(format!("# {}", trimmed));

                    // Prefer the `try_*` configs
                    let actual_key = key.trim_start_matches("try_");
                    if let Some(line_index) = effective_config
                        .iter()
                        .position(|line| line.starts_with(&format!("{}=", key)))
                    {
                        // Config exists, overriding
                        effective_config[line_index] = format!("{}={}", actual_key, value);
                    } else {
                        // Config does not exist, appending
                        effective_config.push(format!("{}={}", key, value)); // Make sure there are no spaces around = so that the check existing key logic works
                    }
                } else {
                    // Keep the config as is
                    write_back_lines.push(trimmed.to_string());

                    if effective_config
                        .iter()
                        .any(|line| line.starts_with(&format!("{}=", key)))
                    {
                        // If already overridden by try_ version, skip inserting
                    } else {
                        // Config does not exist, appending
                        effective_config.push(format!("{}={}", key, value)); // Make sure there are no spaces around = so that the check existing key logic works
                    }
                }
            } else {
                write_back_lines.push(trimmed.to_string());
            }
        }

        // Rewrite config with try_* lines commented out
        let _ = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&full_config_path)
            .and_then(|mut file| {
                for line in &write_back_lines {
                    writeln!(file, "{}", line)?;
                }
                Ok(())
            });
    }

    // Convert effective config back to lines
    effective_config
}

pub fn parse_config() -> LocalConfig {
    let lines = process_config_file();
    let content = lines.join("\n");
    if let Ok(config) = toml::from_str::<LocalConfig>(&content) {
        return config;
    }
    return LocalConfig::default();
}
