// Launch Firefox in kiosk mode (full screen and can't be closed with F11)
//
// The child is stored behind a mutex so the test can stop it when it finishes.
// The browser test runs once per protocol in a session, and a lingering Firefox
// would answer the next run on the protocol it was started with.
pub static BROWSER_INSTANCE: std::sync::LazyLock<std::sync::Mutex<Option<std::process::Child>>> =
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
                // Never hand the URL to an already running instance: the next
                // browser run may need the other protocol.
                command.args(["--no-remote", "--kiosk", &url, "-profile", profile]);
                command
            };

            // Run Firefox on the same protocol as the compiled backend. On a
            // Wayland session the x11 run uses `DISPLAY`, which is the
            // compositor's Xwayland. The browser test is started once per
            // backend, so each invocation gets its own Firefox instance.
            if cfg!(target_os = "linux") {
                if native_wayland() {
                    command.env("GDK_BACKEND", "wayland");
                    command.env("MOZ_ENABLE_WAYLAND", "1");
                } else {
                    command.env("GDK_BACKEND", "x11");
                    command.env("MOZ_ENABLE_WAYLAND", "0");
                }
            }

            command.spawn().expect("Failed to start Firefox")
        };
        std::sync::Mutex::new(Some(child))
    });

/// Stop Firefox so the next browser test starts its own instance.
pub fn kill() {
    let Some(mut child) = BROWSER_INSTANCE
        .lock()
        .expect("browser mutex was poisoned")
        .take()
    else {
        return;
    };
    // SIGKILL does not let Firefox remove its profile lock, and the next run
    // uses the same profile.
    let _ = child.kill();
    let _ = child.wait();
    let profile = std::env::current_dir()
        .expect("failed to get the project root")
        .join("tests/firefox-profile");
    for lock in ["lock", ".parentlock"] {
        let _ = std::fs::remove_file(profile.join(lock));
    }
}

/// Whether the backend compiled into this test binary talks to the compositor
/// natively instead of X11.
///
/// Backend selection is compile-time and `Enigo::new` prefers `xdg_desktop` and
/// `wayland` over `x11`, so those features have to run the browser natively.
/// `libei` is only reached after X11 when both are compiled in.
const fn native_wayland() -> bool {
    cfg!(feature = "xdg_desktop")
        || cfg!(feature = "wayland")
        || (cfg!(feature = "libei") && !cfg!(any(feature = "x11rb", feature = "xdo")))
}

/// Hide the GNOME Shell overview.
///
/// The shell starts in the overview and can show it again late in startup.
/// There is no gsettings key to suppress it, and Escape is one of the keys
/// under test, so use the shell's read/write D-Bus property instead.
#[cfg(target_os = "linux")]
fn hide_overview() {
    let is_gnome =
        std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.contains("GNOME"));
    if !is_gnome {
        return;
    }
    let status = std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Shell",
            "--object-path",
            "/org/gnome/Shell",
            "--method",
            "org.freedesktop.DBus.Properties.Set",
            "org.gnome.Shell",
            "OverviewActive",
            "<false>",
        ])
        .status()
        .expect("failed to run gdbus to hide the GNOME Shell overview");
    assert!(
        status.success(),
        "gdbus could not hide the GNOME Shell overview"
    );
}

/// Give the Firefox window the input focus if the session needs it.
///
/// The X11 window managers in the containers do not activate the kiosk window
/// on their own. `windowactivate` is rejected by the headless GNOME session, so
/// set the X input focus directly. The plain-Xvfb sessions already use
/// `PointerRoot`; focusing explicitly there is harmless.
///
/// A Wayland client cannot focus another client. Kiosk Firefox is a new
/// top-level surface, so the compositor focuses it without help from us.
#[cfg(target_os = "linux")]
pub fn focus() {
    hide_overview();
    if native_wayland() {
        return;
    }
    let Some(display) = std::env::var_os("DISPLAY") else {
        // Outside the devcontainers the runner controls the session and may
        // not have xdotool installed.
        return;
    };
    // Bound the wait so a window that never maps does not block the whole
    // test step. The devcontainers install xdotool and util-linux; other
    // runners control focus themselves.
    let status = match std::process::Command::new("timeout")
        .args([
            "20",
            "xdotool",
            "search",
            "--sync",
            "--onlyvisible",
            "--name",
            "Enigo Universal Test",
            "windowfocus",
            "--sync",
        ])
        .env("DISPLAY", display)
        .status()
    {
        Ok(status) => status,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("timeout/xdotool not installed, not focusing the browser");
            return;
        }
        Err(err) => panic!("failed to run xdotool to focus the Firefox window: {err}"),
    };
    assert!(
        status.success(),
        "xdotool could not focus the Firefox window"
    );
}
