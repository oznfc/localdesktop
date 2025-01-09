use crate::config;
use crate::logging::log_to_panel;
use crate::logging::PolarBearExpectation;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::{collections::VecDeque, io::BufReader, sync::Mutex};

pub mod scaffold;

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Child {
    // On Android, Arch Linux file system was extracted to:
    // /data/data/app.polarbear/files/archlinux

    // Run the command inside Proot

    Command::new("proot")
        .arg("-R")
        .arg(config::ARCH_FS_ROOT)
        .args(command)
        .stdout(Stdio::piped())
        .spawn()
        .pb_expect("Failed to run command")
}

#[cfg(target_os = "macos")]
fn macos_arch_run(command: &[&str]) -> Child {
    // On MacOS, use orb to run the command
    let mut orb_command = vec!["orb"];
    orb_command.extend(command);

    Command::new("orb")
        .arg("-u")
        .arg("root")
        .args(command)
        .stdout(Stdio::piped())
        .spawn()
        .pb_expect("Failed to run command")
}

pub fn arch_run(command: &[&str]) -> Child {
    #[cfg(target_os = "android")]
    return android_arch_run(command);

    #[cfg(target_os = "macos")]
    return macos_arch_run(command);
}

pub fn arch_run_with_log(command: &[&str], logs: &Mutex<VecDeque<String>>) {
    let child = arch_run(command);
    let reader = BufReader::new(child.stdout.pb_expect("Failed to read stdout"));
    for line in reader.lines() {
        let line = line.unwrap();
        log_to_panel(&line, logs);
    }
}
