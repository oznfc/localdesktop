use crate::{
    app::{compositor::PolarBearCompositor, run::PolarBearApp},
    utils::{
        config,
        logging::{log_format, PolarBearExpectation},
    },
};

use super::process::ArchProcess;

pub struct SetupOptions {
    pub install_packages: String,
    pub checking_command: String,
    pub launch_command: String,
    pub username: String,
}

pub fn setup(app: &PolarBearApp, options: SetupOptions) {
    let SetupOptions {
        install_packages,
        checking_command,
        launch_command,
        username,
    } = options;

    let log = |it| {
        app.shared.lock().unwrap().log(it);
    };

    ArchProcess::exec("uname -a").with_log(log);

    // Fix "/tmp" can be written by others
    ArchProcess::exec("chmod 700 /tmp")
        .wait()
        .pb_expect("chmod 700 /tmp failed");

    loop {
        if !ArchProcess::exec(&format!("id {username}"))
            .wait_with_output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            let command = format!("useradd -m -G wheel {username} && passwd -d {username}");
            ArchProcess::exec(&command)
                .wait()
                .pb_expect(&format!("{} failed", command));
        }

        let installed = ArchProcess::exec(&checking_command)
            .wait()
            .pb_expect("Failed to check whether the installation target is installed")
            .success();
        if installed {
            match PolarBearCompositor::build(&app.android_app) {
                Ok(compositor) => {
                    {
                        app.shared.lock().unwrap().compositor.replace(compositor);
                    }
                    let full_launch_command = &format!(
                        "HOME=/home/teddy USER=teddy XDG_RUNTIME_DIR={} WAYLAND_DISPLAY={} XDG_SESSION_TYPE=wayland {} 2>&1",
                        config::XDG_RUNTIME_DIR,
                        config::WAYLAND_SOCKET_NAME,
                        launch_command
                    );
                    ArchProcess::exec(&full_launch_command).with_log(log);
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
            ArchProcess::exec("rm /var/lib/pacman/db.lck");
            ArchProcess::exec(&format!(
                "stdbuf -oL pacman -Syu {} --noconfirm --noprogressbar",
                install_packages
            ))
            .with_log(log);
        }
    }
}
