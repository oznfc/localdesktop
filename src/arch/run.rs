use crate::utils::logging::PolarBearExpectation;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};

#[cfg(target_os = "android")]
use crate::utils::{application_context::get_application_context, config};

#[cfg(target_os = "android")]
fn android_arch_run(command: &str) -> Child {
    // Run the command inside Proot
    let context = get_application_context().pb_expect("Failed to get application context");
    println!("Context inside android_arch_run: {:?}", context);

    Command::new(context.native_library_dir.join("proot.so"))
        .env("PROOT_LOADER", context.native_library_dir.join("loader.so"))
        .env("PROOT_TMP_DIR", context.data_dir.join("files/arch"))
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
        .arg("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .spawn()
        .pb_expect("Failed to run command")
}

#[cfg(all(unix, not(target_os = "android")))]
fn unix_arch_run(command: &str) -> Child {
    // On MacOS, use orb to run the command
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .spawn()
        .pb_expect("Failed to run command")
}

pub fn arch_run(command: &str) -> Child {
    #[cfg(target_os = "android")]
    return android_arch_run(command);

    #[cfg(target_os = "macos")]
    panic!("Initially, to ease development, we use OrbStack to mimic PRoot behavior on MacOS. However, you cannot bind an UNIX socket from MacOS and connect to it from the OrbStack Linux machine, since UNIX sockets require kernel support. SHM may not work as well. Luckily, we found a way to debug Rust code running directly on Android, so MacOS specific code is not needed anymore.");

    #[cfg(all(unix, not(target_os = "android")))]
    return unix_arch_run(command); // On Unix, we can use the host directly.

    panic!("Unsupported OS! Please run on Android/Unix.");
}

pub fn arch_run_with_log<T: FnMut(String)>(command: &str, mut log: T) {
    let child = arch_run(command);
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
