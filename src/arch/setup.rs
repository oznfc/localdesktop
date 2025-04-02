use super::process::ArchProcess;
use crate::{
    app::compositor::PolarBearCompositor,
    utils::{config, logging::PolarBearExpectation},
};
use winit::platform::android::activity::AndroidApp;

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
            ArchProcess::exec("rm -f /var/lib/pacman/db.lck"); // Install dependencies
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
    // Clean up potential leftover files for display :1
    ArchProcess::exec("rm -f /tmp/.X1-lock");
    ArchProcess::exec("rm -f /tmp/.X11-unix/X1");

    let full_launch_command = &format!(
        "XDG_RUNTIME_DIR={} Xwayland -hidpi :1 2>&1 & \
        while [ ! -e /tmp/.X11-unix/X1 ]; do sleep 0.1; done; \
        XDG_SESSION_TYPE=x11 DISPLAY=:1 {} 2>&1",
        config::XDG_RUNTIME_DIR,
        launch_command
    );
    ArchProcess::exec(&full_launch_command).with_log(|it| {
        println!("{}", it);
    });
}
