use crate::application_context::get_application_context;
use crate::config;
use crate::logging::{log_to_panel, PolarBearExpectation};
use egui_winit::winit::platform::android::activity::AndroidApp;
use std::collections::VecDeque;
use std::fs;
use std::sync::{Arc, Mutex};
use tar::Archive;
use xz2::read::XzDecoder;

pub fn scaffold(android_app: &AndroidApp, logs: &Arc<Mutex<VecDeque<String>>>) {
    let context = get_application_context().pb_expect("Failed to get application context");
    println!("Application context: {:?}", context);
    let fs_root = std::path::Path::new(config::ARCH_FS_ROOT);
    let tar_file = android_app
        .asset_manager()
        .open(
            std::ffi::CString::new(config::ARCH_FS_ARCHIVE)
                .pb_expect("Failed to create CString from ARCH_FS_ARCHIVE")
                .as_c_str(),
        )
        .pb_expect("Failed to open Arch Linux FS .tar.xz in asset manager");

    let mut should_pacstrap = false;
    if !fs_root.exists()
        || fs::read_dir(fs_root)
            .pb_expect("Failed to read fs_root directory")
            .next()
            .is_none()
    {
        should_pacstrap = true;
        log_to_panel("Arch Linux is not installed! Installing...", logs);
    }

    if should_pacstrap {
        log_to_panel("(This may take a few minutes.)", logs);

        // Ensure the extracted directory is clean
        let extracted_dir = &context.data_dir.join("archlinux-aarch64");
        fs::remove_dir_all(extracted_dir).unwrap_or(());

        // Extract tar file directly to the final destination
        let tar = XzDecoder::new(tar_file);
        let mut archive = Archive::new(tar);
        archive
            .unpack(context.data_dir.clone())
            .pb_expect("Failed to extract Arch Linux FS .tar.xz file");

        // Move the extracted files to the final destination
        fs::rename(extracted_dir, fs_root)
            .pb_expect("Failed to rename extracted files to final destination");
    }
}
