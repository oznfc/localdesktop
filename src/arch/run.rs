use crate::utils::logging::{log_to_panel, PolarBearExpectation};
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::{collections::VecDeque, io::BufReader, sync::Mutex};

#[cfg(target_os = "android")]
use crate::utils::{application_context::get_application_context, config};

#[cfg(target_os = "android")]
fn android_arch_run(command: &[&str]) -> Child {
    // Run the command inside Proot
    let context = get_application_context().pb_expect("Failed to get application context");
    println!("Context inside android_arch_run: {:?}", context);

    Command::new(context.native_library_dir.join("proot.so"))
        .arg("-r")
        .arg(config::ARCH_FS_ROOT)
        .arg("-L")
        .arg("--link2symlink")
        .arg("--kill-on-exit")
        .arg("--root-id")
        .arg("--cwd=/root")
        .arg("--bind=/dev")
        // .arg("--bind=\"/dev/urandom:/dev/random\"")
        .arg("--bind=/proc")
        // .arg("--bind=\"/proc/self/fd:/dev/fd\"")
        // .arg("--bind=\"/proc/self/fd/0:/dev/stdin\"")
        // .arg("--bind=\"/proc/self/fd/1:/dev/stdout\"")
        // .arg("--bind=\"/proc/self/fd/2:/dev/stderr\"")
        .arg("--bind=/sys")
        // .arg("--bind=\"${rootFs}/proc/.loadavg:/proc/loadavg\"")
        // .arg("--bind=\"${rootFs}/proc/.stat:/proc/stat\"")
        // .arg("--bind=\"${rootFs}/proc/.uptime:/proc/uptime\"")
        // .arg("--bind=\"${rootFs}/proc/.version:/proc/version\"")
        // .arg("--bind=\"${rootFs}/proc/.vmstat:/proc/vmstat\"")
        // .arg("--bind=\"${rootFs}/proc/.sysctl_entry_cap_last_cap:/proc/sys/kernel/cap_last_cap\"")
        // .arg("--bind=\"${rootFs}/sys/.empty:/sys/fs/selinux\"")
        .arg("/usr/bin/env")
        .arg("-i")
        .arg("\"HOME=/root\"")
        .arg("\"LANG=C.UTF-8\"")
        .arg("\"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin\"")
        .arg("\"TMPDIR=/tmp\"")
        .args(command)
        .env("PROOT_LOADER", context.native_library_dir.join("loader.so"))
        .env(
            "PROOT_TMP_DIR",
            context.data_dir.join("files/archlinux-aarch64"),
        )
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
    fn should_echoable() {
        let command = &["echo", "hello"];
        let child = arch_run(command);
        let output = child.wait_with_output().expect("Failed to read output");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn should_output_uname() {
        let command = &["uname", "-a"];
        let child = arch_run(command);
        let output = child.wait_with_output().expect("Failed to read output");
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
        assert!(String::from_utf8_lossy(&output.stdout)
            .to_lowercase()
            .contains("arch"));
    }

    #[test]
    fn should_run_with_log_successfully() {
        let command = &["echo", "hello"];
        let logs = Mutex::new(VecDeque::new());
        arch_run_with_log(command, &logs);
        let logs = logs.lock().unwrap();
        assert!(logs.iter().any(|log| log.contains("hello")));
    }

    #[test]
    fn should_exit_with_success_code() {
        let command = &["pacman", "-Ss", "chrome"];
        let mut child = arch_run(command);
        let status = child.wait().expect("Failed to wait for child");
        assert_eq!(status.success(), true);
    }

    #[test]
    fn should_exit_with_fail_code() {
        let command = &["pacman", "-Qg", "plasmma"]; // notice the typo
        let mut child = arch_run(command);
        let status = child.wait().expect("Failed to wait for child");
        assert_ne!(status.success(), true);
    }
}
