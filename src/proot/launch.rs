use super::process::ArchProcess;
use crate::utils::config;

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
