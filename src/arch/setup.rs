use winit::platform::android::activity::AndroidApp;

use crate::{
    app::compositor::PolarBearCompositor,
    utils::{config, logging::PolarBearExpectation},
};

use super::process::ArchProcess;

pub struct SetupOptions {
    pub install_packages: String,
    pub checking_command: String,
    pub username: String,
    pub log: Box<dyn Fn(String)>,
    pub android_app: AndroidApp,
}

pub fn setup(options: SetupOptions) -> PolarBearCompositor {
    let SetupOptions {
        install_packages,
        checking_command,
        username,
        log,
        android_app,
    } = options;

    ArchProcess::exec("uname -a").with_log(&log);

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
            break; // Start the compositor now
        } else {
            ArchProcess::exec("rm /var/lib/pacman/db.lck"); // Install dependencies
            ArchProcess::exec(&format!(
                "stdbuf -oL pacman -Syu {} --noconfirm --noprogressbar",
                install_packages
            ))
            .with_log(&log);
        }
    }

    let compositor = PolarBearCompositor::build().pb_expect("Failed to build compositor");
    compositor
}

pub fn launch(launch_command: String) {
    let full_launch_command = &format!(
        "HOME=/home/teddy USER=teddy XDG_RUNTIME_DIR={} WAYLAND_DISPLAY={} XDG_SESSION_TYPE=wayland {} 2>&1",
        config::XDG_RUNTIME_DIR,
        config::WAYLAND_SOCKET_NAME,
        launch_command
    );
    ArchProcess::exec(&full_launch_command).with_log(|it| {
        println!("{}", it);
    });
}
