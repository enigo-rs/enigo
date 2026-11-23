use std::process::{Child, Command};

/// A Firefox process that is stopped when the value is dropped.
///
/// Dropping the value also runs while a test unwinds from a panic, so a failed
/// test does not leave a browser behind for the next run.
pub struct Browser(Child);

impl Browser {
    /// Launch Firefox in kiosk mode (full screen and can't be closed with F11).
    ///
    /// Firefox inherits `GDK_BACKEND` from the runner, which selects Wayland or
    /// X11.
    pub fn launch() -> Self {
        // Project root
        let cwd = std::env::current_dir().unwrap();

        // Paths
        let index = cwd.join("tests/index.html");
        let url = format!("file://{}", index.to_str().unwrap());
        let profile = cwd.join("tests/firefox-profile");
        let profile = profile.to_str().unwrap();

        let mut command = if cfg!(target_os = "windows") {
            let mut command = Command::new("cmd");
            command.args([
                "/C", "start", "firefox", "--kiosk", &url, "-profile", profile,
            ]);
            command
        } else if cfg!(target_os = "macos") {
            let mut command = Command::new("open");
            command.args([
                "-a", "Firefox", "--args", "--kiosk", &url, "-profile", profile,
            ]);
            command
        } else {
            let mut command = Command::new("firefox");
            // Never hand the URL to an already running instance: the next
            // browser run may need the other protocol.
            command.args(["--no-remote", "--kiosk", &url, "-profile", profile]);
            command
        };

        Self(command.spawn().expect("Failed to start Firefox"))
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        // SIGKILL does not let Firefox remove its profile lock, and the next
        // run uses the same profile.
        let _ = self.0.kill();
        let _ = self.0.wait();
        if let Ok(cwd) = std::env::current_dir() {
            for lock in ["lock", ".parentlock"] {
                let _ = std::fs::remove_file(cwd.join("tests/firefox-profile").join(lock));
            }
        }
    }
}
