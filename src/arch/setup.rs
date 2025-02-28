use std::sync::{Arc, Mutex};

use egui_winit::winit::platform::android::activity::AndroidApp;

use crate::{
    app::{
        compositor::PolarBearCompositor,
        run::Shared,
    },
    utils::{
        config,
        logging::{log_format, PolarBearExpectation},
    },
};

use super::run::{arch_run, arch_run_as_with_log, arch_run_with_log};

pub struct DesktopOption<'a> {
    pub android_app: &'a AndroidApp,
    pub shared: Arc<Mutex<Shared>>,
    pub package_group: &'a str,
    pub launch_command: &'a str,
    pub username: &'a str,
}

pub fn check_install_and_launch(options: DesktopOption) {
    let DesktopOption {
        android_app,
        shared,
        package_group,
        launch_command,
        username,
    } = options;

    let log = |it| {
        shared.lock().unwrap().log(it);
    };

    loop {
        if !arch_run(&format!("id {username}"))
            .wait_with_output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            let command = format!("useradd -m -G wheel {username} && passwd -d {username}");
            arch_run(&command)
                .wait()
                .pb_expect(&format!("{} failed", command));
        }

        let install_command = format!("pacman -Qg {}", package_group);
        let installed = arch_run(&install_command)
            .wait()
            .pb_expect(&format!("{} failed", install_command))
            .success();
        if installed {
            match PolarBearCompositor::build(android_app) {
                Ok(compositor) => {
                    {
                        shared.lock().unwrap().compositor.replace(compositor);
                    }
                    arch_run_as_with_log(
            &format!(
                    "HOME=/home/teddy USER=teddy XDG_RUNTIME_DIR={} WAYLAND_DISPLAY={} WAYLAND_DEBUG=client {} 2>&1",
                    config::XDG_RUNTIME_DIR,
                    config::WAYLAND_SOCKET_NAME,
                    launch_command),
                    
            username,
            log,
        );
                }
                Err(e) => {
                    log(log_format(
                        "POLAR BEAR COMPOSITOR RUNTIME ERROR",
                        &format!("{}", e),
                    ));
                }
            }
            break;
        } else {
            arch_run("rm /var/lib/pacman/db.lck");
            arch_run_with_log(&format!("pacman -Syu {} --noconfirm", package_group), log);
        }
    }
}
