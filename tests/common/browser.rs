// Launch Firefox in kiosk mode (full screen and can't be closed with F11)
//
// n.b. static items do not call [`Drop`] on program termination, so this won't
// be deallocated. this is fine, as the OS can deallocate the terminated program
// faster than we can free memory but tools like valgrind might report "memory
// leaks" as it isn't obvious this is intentional.
pub static BROWSER_INSTANCE: std::sync::LazyLock<Option<std::process::Child>> =
    std::sync::LazyLock::new(|| {
        // Project root
        let cwd = std::env::current_dir().unwrap();

        // Paths
        let index = cwd.join("tests/index.html");
        let url = format!("file://{}", index.to_str().unwrap());
        let profile = cwd.join("tests/firefox-profile");
        let profile = profile.to_str().unwrap();

        let child = {
            let mut command = if cfg!(target_os = "windows") {
                let mut command = std::process::Command::new("cmd");
                command.args([
                    "/C", "start", "firefox", "--kiosk", &url, "-profile", profile,
                ]);
                command
            } else if cfg!(target_os = "macos") {
                let mut command = std::process::Command::new("open");
                command.args([
                    "-a", "Firefox", "--args", "--kiosk", &url, "-profile", profile,
                ]);
                command
            } else {
                let mut command = std::process::Command::new("firefox");
                command.args(["--kiosk", &url, "-profile", profile]);
                command
            };

            // The devcontainers run Firefox on a dedicated X11 display, also
            // in Wayland sessions. Without that variable, keep the desktop's
            // native backend.
            if cfg!(target_os = "linux") {
                if let Some(display) = std::env::var_os("ENIGO_BROWSER_DISPLAY") {
                    command.env("GDK_BACKEND", "x11");
                    command.env("MOZ_ENABLE_WAYLAND", "0");
                    command.env("DISPLAY", display);
                }
            }

            command.spawn().expect("Failed to start Firefox")
        };
        Some(child)
    });
