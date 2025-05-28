use crate::utils::logging::PolarBearExpectation;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Child, Command, Stdio};

use crate::utils::{application_context::get_application_context, config};

pub type Log = Box<dyn Fn(String)>;

pub struct ArchProcess {
    pub command: String,
    pub user: String,
    pub process: Option<Child>,
}

impl ArchProcess {
    pub fn spawn(mut self) -> Self {
        // Run the command inside Proot
        let context = get_application_context().pb_expect("Failed to get application context");

        #[cfg(not(test))]
        let proot_loader = context.native_library_dir.join("libproot_loader.so");
        #[cfg(test)]
        let proot_loader = "/data/local/tmp/libproot_loader.so";

        let mut process = Command::new(context.native_library_dir.join("libproot.so"));
        process
            .env("PROOT_LOADER", proot_loader)
            .env("PROOT_TMP_DIR", config::ARCH_FS_ROOT)
            .arg("-r")
            .arg(config::ARCH_FS_ROOT)
            .arg("-L")
            .arg("--link2symlink")
            .arg("--sysvipc")
            .arg("--kill-on-exit")
            .arg("--root-id")
            .arg("--cwd=/root")
            .arg("--bind=/dev")
            .arg("--bind=/proc")
            .arg("--bind=/sys")
            .arg(format!("--bind={}/tmp:/dev/shm", config::ARCH_FS_ROOT))
            .arg("/usr/bin/env")
            .arg("-i")
            .arg("\"HOME=/root\"")
            .arg("\"LANG=C.UTF-8\"")
            .arg("\"PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/local/games:/usr/games:/system/bin:/system/xbin\"")
            .arg("\"TMPDIR=/tmp\"");
        if self.user == "root" {
            process.arg("sh");
        } else {
            process.arg("su").arg("-").arg(&self.user);
        }
        let child = process
            .arg("-c")
            .arg(&self.command)
            .stdout(Stdio::piped())
            .spawn()
            .pb_expect("Failed to run command");

        self.process.replace(child);
        self
    }

    pub fn exec(command: &str) -> Self {
        ArchProcess {
            command: command.to_string(),
            user: "root".to_string(),
            process: None,
        }
        .spawn()
    }

    pub fn exec_as(command: &str, user: &str) -> Self {
        ArchProcess {
            command: command.to_string(),
            user: user.to_string(),
            process: None,
        }
        .spawn()
    }

    pub fn with_log(self, mut log: impl FnMut(String)) {
        if let Some(child) = self.process {
            let reader = BufReader::new(child.stdout.unwrap());
            for line in reader.lines() {
                let line = line.unwrap();
                log(line);
            }
        }
    }

    pub fn wait_with_output(self) -> std::io::Result<std::process::Output> {
        if let Some(child) = self.process {
            child.wait_with_output()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Process not spawned",
            ))
        }
    }

    pub fn wait(self) -> std::io::Result<std::process::ExitStatus> {
        if let Some(mut child) = self.process {
            child.wait()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Process not spawned",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn should_echoable() {
        let process = ArchProcess::exec("echo hello");
        let output = process.wait_with_output().expect("Failed to read output");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn should_output_uname() {
        let process = ArchProcess::exec("uname -a");
        let output = process.wait_with_output().expect("Failed to read output");
        println!("Output: {}", String::from_utf8_lossy(&output.stdout));
        assert!(String::from_utf8_lossy(&output.stdout)
            .to_lowercase()
            .contains("arch"));
    }

    #[test]
    fn should_run_with_log_successfully() {
        let mut logs = VecDeque::new();
        ArchProcess {
            command: "echo hello".to_string(),
            user: "root".to_string(),
            process: None,
        }
        .spawn()
        .with_log(|log| {
            logs.push_back(log.to_string());
        });
        assert!(logs.iter().any(|log| log.contains("hello")));
    }

    #[test]
    fn should_exit_with_success_code() {
        let process = ArchProcess::exec("pacman -Ss chrome");
        let status = process.wait().expect("Failed to wait for process");
        assert_eq!(status.success(), true);
    }

    #[test]
    fn should_exit_with_fail_code() {
        let process = ArchProcess::exec("pacman -Qg plasmma");
        let status = process.wait().expect("Failed to wait for process");
        assert_ne!(status.success(), true);
    }
}
