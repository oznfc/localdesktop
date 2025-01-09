use crate::config;
use crate::logging::{log_format, log_to_panel};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tar::Archive;
use xz2::read::XzDecoder;

#[cfg(target_os = "android")]
use crate::application_context::ApplicationContext;

#[cfg(target_os = "android")]
pub fn scaffold(context: &ApplicationContext, logs: &Arc<Mutex<VecDeque<String>>>) {
    let fs_root = std::path::Path::new(config::ARCH_FS_ROOT);
    let tar_file = context.data_dir.join(config::ARCH_FS_ARCHIVE);
    let temp_dir = context
        .data_dir
        .join(fs_root.file_name().unwrap().to_str().unwrap().to_owned() + ".lock");

    let mut should_pacstrap = false;
    if !fs_root.exists() || fs::read_dir(fs_root).unwrap().next().is_none() {
        should_pacstrap = true;
        log_to_panel(
            &log_format("INFO", "Arch Linux is not installed! Installing..."),
            logs,
        );
    }

    if should_pacstrap {
        log_to_panel(&log_format("INFO", "(This may take a few minutes.)"), logs);

        // Ensure the temporary directory is clean
        fs::remove_dir_all(&temp_dir).unwrap_or(());
        fs::create_dir_all(&temp_dir).unwrap();

        // Extract tar file here
        let tar_gz = File::open(tar_file).unwrap();
        let tar = XzDecoder::new(BufReader::new(tar_gz));
        let mut archive = Archive::new(tar);
        archive.unpack(&temp_dir).unwrap();

        // Move extracted files to the final destination
        fs::remove_dir_all(fs_root).unwrap_or(());
        fs::rename(&temp_dir, fs_root).unwrap();
    }
}
