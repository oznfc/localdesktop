use crate::utils::logging::PolarBearExpectation;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};

use crate::utils::{application_context::get_application_context, config};

pub fn arch_run_as(command: &str, user: &str) -> Child {
    // Run the command inside Proot
    let context = get_application_context().pb_expect("Failed to get application context");

    #[cfg(not(test))]
    let proot_loader = context.native_library_dir.join("loader.so");
    #[cfg(test)]
    let proot_loader = "/data/local/tmp/loader.so";

    let mut process = Command::new(context.native_library_dir.join("proot.so"));
    process
        .env("PROOT_LOADER", proot_loader)
        .env("PROOT_TMP_DIR", config::ARCH_FS_ROOT)
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
        .arg("\"TMPDIR=/tmp\"");
    if user == "root" {
        process.arg("sh");
    } else {
        process.arg("su").arg("-").arg(user);
    }
    process
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .spawn()
        .pb_expect("Failed to run command")
}

pub fn arch_run(command: &str) -> Child {
    arch_run_as(command, "root")
}

pub fn arch_run_with_log<T: FnMut(String)>(command: &str, mut log: T) {
    let child = arch_run(command);
    let reader = BufReader::new(child.stdout.pb_expect("Failed to read stdout"));
    for line in reader.lines() {
        let line = line.pb_expect("Failed to read line");
        log(line);
    }
}

pub fn arch_run_as_with_log<T: FnMut(String)>(command: &str, user: &str, mut log: T) {
    let child = arch_run_as(command, user);
    let reader = BufReader::new(child.stdout.pb_expect("Failed to read stdout"));
    for line in reader.lines() {
        let line = line.pb_expect("Failed to read line");
        log(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn should_echoable() {
        let child = arch_run("echo hello");
        let output = child.wait_with_output().expect("Failed to read output");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn should_output_uname() {
        let child = arch_run("uname -a");
        let output = child.wait_with_output().expect("Failed to read output");
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
        assert!(String::from_utf8_lossy(&output.stdout)
            .to_lowercase()
            .contains("arch"));
    }

    #[test]
    fn should_run_with_log_successfully() {
        let mut logs = VecDeque::new();
        arch_run_with_log("echo hello", |log| {
            logs.push_back(log.to_string());
        });
        assert!(logs.iter().any(|log| log.contains("hello")));
    }

    #[test]
    fn should_exit_with_success_code() {
        let mut child = arch_run("pacman -Ss chrome");
        let status = child.wait().expect("Failed to wait for child");
        assert_eq!(status.success(), true);
    }

    #[test]
    fn should_exit_with_fail_code() {
        let mut child = arch_run("pacman -Qg plasmma");
        let status = child.wait().expect("Failed to wait for child");
        assert_ne!(status.success(), true);
    }
}
