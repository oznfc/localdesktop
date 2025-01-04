use std::process::{Child, Command, Stdio};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader},
    sync::{Arc, Mutex},
};

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Child {
    // On Android, Arch Linux file system was extracted to:
    // /data/data/app.polarbear/files/archlinux

    let arch_fs = "/data/data/app.polarbear/files/archlinux";

    // Run the command inside Proot
    let mut proot_command = vec!["proot", arch_fs];
    proot_command.extend(command);

    Command::new("proot")
        .arg("-R")
        .arg(arch_fs)
        .args(command)
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run command")
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
        .expect("Failed to run command")
}

pub fn arch_run(command: &[&str]) -> Child {
    #[cfg(target_os = "android")]
    return android_arch_run(command);

    #[cfg(target_os = "macos")]
    return macos_arch_run(command);
}

pub fn arch_run_with_log(command: &[&str], logs: &Arc<Mutex<VecDeque<String>>>) {
    let child = arch_run(command);
    let reader = BufReader::new(child.stdout.expect("Failed to read stdout"));
    for line in reader.lines() {
        let line: String = line.unwrap();
        let mut logs = logs.lock().unwrap();
        logs.push_back(line);
        // Ensure the logs size stays at most 20
        if logs.len() > 20 {
            logs.pop_front();
        }
    }
}
