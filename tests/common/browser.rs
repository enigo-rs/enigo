use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

/// A Firefox instance that is stopped on drop, including on test failure.
///
/// On Linux the child is Firefox itself; macOS and Windows use a launcher.
pub struct Browser {
    child: Child,
    /// Unique profile passed to Firefox. On macOS it identifies the browser,
    /// because the child is only the `open` launcher.
    profile: String,
    /// Where Firefox's stdout/stderr go. A file instead of a pipe, so a
    /// browser outliving the test cannot keep cargo's output open. CI uploads
    /// the `ci-logs/` directory on failure.
    log_path: PathBuf,
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
        // cargo's pipe (and the CI step) open. A log file keeps the output for
        // diagnosing why Firefox did not connect.
        let log_dir = cwd.join("ci-logs");
        std::fs::create_dir_all(&log_dir).expect("Failed to create the ci-logs directory");
        let log_path = log_dir.join(format!("firefox-{}.log", std::process::id()));
        let log = File::create(&log_path).expect("Failed to create the Firefox log file");
        let log_err = log
            .try_clone()
            .expect("Failed to clone the Firefox log file handle");
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err));

        let child = command.spawn().expect("Failed to start Firefox");
        log::debug!(
            "Firefox started, logging its output to {}",
            log_path.display()
        );
        Self {
            child,
            profile,
            log_path,
        }
    }

    /// Returns the exit status if the browser process has already exited.
    ///
    /// Only meaningful on Linux, where the child is Firefox itself. On macOS
    /// and Windows the child is only a launcher that exits right after handing
    /// the launch off, so its exit says nothing about the browser.
    pub fn exited(&mut self) -> Option<ExitStatus> {
        #[cfg(target_os = "linux")]
        {
            match self.child.try_wait() {
                Ok(status) => status,
                Err(e) => {
                    log::warn!("failed to query the Firefox process: {e}");
                    None
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// The tail of Firefox's output, trimmed to keep panic messages readable.
    pub fn log(&self) -> String {
        let output = std::fs::read(&self.log_path).unwrap_or_default();
        let start = output.len().saturating_sub(8 * 1024);
        String::from_utf8_lossy(&output[start..]).into_owned()
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
