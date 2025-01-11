use crate::config;
use crate::logging::log_to_panel;
use crate::logging::PolarBearExpectation;
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::{collections::VecDeque, io::BufReader, sync::Mutex};

#[cfg(target_os = "android")]
pub mod scaffold;

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Child {
    // On Android, Arch Linux file system was extracted to:
    // /data/data/app.polarbear/files/archlinux

    // Run the command inside Proot

    use crate::application_context::get_application_context;
    let context = get_application_context().pb_expect("Failed to get application context");
    println!("Context inside android_arch_run: {:?}", context);

    Command::new(context.native_library_dir.join("proot-rs.so"))
        .arg("-r")
        .arg(config::ARCH_FS_ROOT)
        .arg(command.join(" "))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[test]
    fn test_arch_run_android() {
        #[cfg(target_os = "android")]
        {
            let command = &["echo", "hello"];
            let child = arch_run(command);
            let output = child.wait_with_output().expect("Failed to read output");
            assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
        }
    }

    #[test]
    fn test_arch_run_macos() {
        #[cfg(target_os = "macos")]
        {
            let command = &["echo", "hello"];
            let child = arch_run(command);
            let output = child.wait_with_output().expect("Failed to read output");
            assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
        }
    }

    #[test]
    fn test_arch_run_with_log() {
        let command = &["echo", "hello"];
        let logs = Mutex::new(VecDeque::new());
        arch_run_with_log(command, &logs);
        let logs = logs.lock().unwrap();
        assert!(logs.iter().any(|log| log.contains("hello")));
    }
}
