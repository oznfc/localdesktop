use std::{error::Error, process::Command};

pub fn process(command: &[&str]) -> Result<String, Box<dyn Error>> {
    let [program, args @ ..] = command else {
        panic!("Invalid command {:?}", command);
    };
    let output = Command::new(program)
        .args(args)
        .output()
        .expect(&format!("Cannot spawn {} {:?}", program, args));

    let stdout = String::from_utf8(output.stdout).expect("Failed to read from stdout");
    let stderr = String::from_utf8(output.stderr).expect("Failed to read from stderr");
    if stderr.is_empty() {
        Ok(stdout)
    } else {
        panic!(
            "Command execution failed: {} {:?} {}",
            program, args, stderr
        );
    }
}

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Result<String, Box<dyn Error>> {
    // On Android, Arch Linux file system was extracted to:
    // /data/data/app.polarbear/files/archlinux
    let arch_fs = "/data/data/app.polarbear/files/archlinux";

    // Run the command inside Proot
    let mut proot_command = vec!["proot", arch_fs];
    proot_command.extend(command);
    process(&proot_command)
}

#[cfg(target_os = "macos")]
fn macos_arch_run(command: &[&str]) -> Result<String, Box<dyn Error>> {
    // On MacOS, use orb to run the command
    let mut orb_command = vec!["orb"];
    orb_command.extend(command);
    process(&orb_command)
}

pub fn arch_run(command: &[&str]) -> Result<String, Box<dyn Error>> {
    #[cfg(target_os = "android")]
    return android_arch_run(command);

    #[cfg(target_os = "macos")]
    return macos_arch_run(command);
}

pub fn boot() {
    println!("{}", arch_run(&["uname", "-a"]).unwrap());
}
