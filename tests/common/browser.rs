use std::process::{Child, Command, Stdio};

/// A Firefox instance that is stopped on drop, including on test failure.
///
/// On Linux the child is Firefox itself; macOS and Windows use a launcher.
pub struct Browser {
    child: Child,
    /// Unique profile passed to Firefox. On macOS it identifies the browser,
    /// because the child is only the `open` launcher.
    profile: String,
}

impl Browser {
    /// Launch Firefox in kiosk mode. Firefox inherits `GDK_BACKEND` from the
    /// runner, which selects Wayland or X11.
    pub fn launch() -> Self {
        // Project root
        let cwd = std::env::current_dir().unwrap();

        // Paths
        let index = cwd.join("tests/index.html");
        let url = format!("file://{}", index.to_str().unwrap());
        let profile = cwd.join("tests/firefox-profile");
        let profile = profile.to_str().unwrap().to_owned();

        // Firefox is single-instance per profile; `--new-instance` keeps the
        // test browser separate from a running instance.
        let mut command = if cfg!(target_os = "windows") {
            let mut command = Command::new("cmd");
            command.args([
                "/C",
                "start",
                "firefox",
                "--new-instance",
                "--kiosk",
                &url,
                "-profile",
                &profile,
            ]);
            command
        } else if cfg!(target_os = "macos") {
            let mut command = Command::new("open");
            // `-n` forces a new process; otherwise `open` only activates a
            // running Firefox and drops `--args`.
            command.args([
                "-n",
                "-a",
                "Firefox",
                "--args",
                "--new-instance",
                "--kiosk",
                &url,
                "-profile",
                &profile,
            ]);
            command
        } else {
            let mut command = Command::new("firefox");
            command.args(["--new-instance", "--kiosk", &url, "-profile", &profile]);
            command
        };

        // Don't inherit stdout/stderr: a browser outliving the test would keep
        // cargo's pipe (and the CI step) open.
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command.spawn().expect("Failed to start Firefox");
        Self { child, profile }
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        // Kills Firefox on Linux, where it is the child. On macOS and Windows
        // this only reaches the launcher; macOS stops the browser below.
        let _ = self.child.kill();
        let _ = self.child.wait();

        // `open` exits after handing the launch to LaunchServices, so stop the
        // browser by its profile instead.
        #[cfg(target_os = "macos")]
        {
            let _ = Command::new("pkill")
                .args(["-f", &self.profile])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }

        // SIGKILL leaves the profile lock behind; the next run reuses it.
        for lock in ["lock", ".parentlock"] {
            let _ = std::fs::remove_file(std::path::Path::new(&self.profile).join(lock));
        }
    }
}
