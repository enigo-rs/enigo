// Launch Firefox in kiosk mode (full screen and can't be closed with F11)
//
// n.b. static items do not call [`Drop`] on program termination, so this won't
// be deallocated. this is fine, as the OS can deallocate the terminated program
// faster than we can free memory but tools like valgrind might report "memory
// leaks" as it isn't obvious this is intentional.
pub static BROWSER_INSTANCE: std::sync::LazyLock<Option<std::process::Child>> =
    std::sync::LazyLock::new(|| {
        // Construct the URL
        let url = format!(
            "file://{}/tests/index.html",
            std::env::current_dir().unwrap().to_str().unwrap()
        );

        let child = if cfg!(target_os = "windows") {
            // On Windows, use cmd.exe to run the "start" command
            std::process::Command::new("cmd")
                .args(["/C", "start", "firefox", "--kiosk", &url])
                .spawn()
                .expect("Failed to start Firefox on Windows")
        } else {
            std::process::Command::new("firefox")
                .args(["--kiosk", &url])
                .spawn()
                .expect("Failed to start Firefox")
        };
        Some(child)
    });
