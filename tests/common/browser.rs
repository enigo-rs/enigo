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

        let child = if cfg!(target_os = "windows") {
            // On Windows, use cmd.exe to run the "start" command
            std::process::Command::new("cmd")
                .args([
                    "/C", "start", "firefox", "--kiosk", &url, "-profile", profile,
                ])
                .spawn()
                .expect("Failed to start Firefox")
        } else if cfg!(target_os = "macos") {
            // On macOS, use the "open" command to run Firefox
            std::process::Command::new("open")
                .args([
                    "-a", "Firefox", "--args", "--kiosk", &url, "-profile", profile,
                ])
                .spawn()
                .expect("Failed to start Firefox")
        } else {
            // On Linux, use the "firefox" command
            std::process::Command::new("firefox")
                .args(["--kiosk", &url, "-profile", profile])
                .spawn()
                .expect("Failed to start Firefox")
        };
        Some(child)
    });
