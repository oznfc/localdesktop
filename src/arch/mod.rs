use std::io::Error;
use std::process::{Child, Command, Stdio};

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Result<Child, Error> {
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
}

#[cfg(target_os = "macos")]
fn macos_arch_run(command: &[&str]) -> Result<Child, Error> {
    // On MacOS, use orb to run the command
    let mut orb_command = vec!["orb"];
    orb_command.extend(command);

    Command::new("orb")
        .args(command)
        .stdout(Stdio::piped())
        .spawn()
}

pub fn arch_run(command: &[&str]) -> Result<Child, Error> {
    #[cfg(target_os = "android")]
    return android_arch_run(command);

    #[cfg(target_os = "macos")]
    return macos_arch_run(command);
}
