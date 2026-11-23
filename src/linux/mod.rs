use log::{debug, error, trace, warn};

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse,
    NewConError, Settings,
};

// If none of these features is enabled, there is no way to simulate input
#[cfg(not(any(
    feature = "wayland",
    feature = "x11rb",
    feature = "xdo",
    feature = "libei",
    feature = "xdg_desktop"
)))]
compile_error!(
    "either feature `wayland`, `x11rb`, `xdo` or `libei` must be enabled for this crate when using linux"
);

#[cfg(all(
    any(feature = "tokio", feature = "smol"),
    not(any(feature = "libei", feature = "xdg_desktop"))
))]
compile_error!(
    "You activated a feature (`tokio` or `smol`) to provide an async runtime but did not activate a protocol that needs it"
);

#[cfg(all(feature = "tokio", feature = "smol"))]
compile_error!(
    "the features `tokio` and `smol` are mutually exclusive! You have to chose which async runtime you want to use"
);

#[cfg(all(feature = "xdg_desktop", not(any(feature = "tokio", feature = "smol"))))]
compile_error!(
    "the xdg_desktop feature can only be used if either feature `tokio` or `smol` is enabled to provide an async runtime"
);

#[cfg(all(feature = "libei", not(any(feature = "tokio", feature = "smol"))))]
compile_error!(
    "the libei feature can only be used if either feature `tokio` or `smol` is enabled to provide an async runtime"
);

/// `xdg-desktop-portal` 1.20 exposes version 2 of the `RemoteDesktop`
/// interface, which adds restore tokens and `ConnectToEIS`. Older portals
/// either ignore the restore token or cannot provide an EIS connection, which
/// makes a headless `Enigo::new` wait for a consent dialog forever.
#[cfg(any(feature = "libei", feature = "xdg_desktop"))]
const MIN_REMOTE_DESKTOP_VERSION: u32 = 2;

/// Bound the cost of parsing a keymap received over the wire (from a Wayland
/// compositor or the EIS). This protects us from a buggy or malicious peer
/// sending an unbounded keymap.
#[cfg(any(feature = "wayland", feature = "libei"))]
const MAX_KEYMAP_SIZE: usize = 4 * 1024 * 1024;

/// The largest keycode we accept or generate. Scans over a keymap are clamped
/// to this so a keymap that declares an absurd `maximum` cannot make them
/// unbounded, and it matches the `u16` keycodes the public `raw` API accepts.
#[cfg(any(feature = "wayland", feature = "x11rb", feature = "libei"))]
const MAX_KEYCODE: u32 = u16::MAX as u32;

/// Driving the `ashpd`/`zbus` portal futures from synchronous code.
///
/// With the `tokio` backend, `zbus` spawns the D-Bus connection's socket
/// reader with `tokio::spawn`, which attaches it to the runtime that created
/// the connection. Building a fresh runtime for every call and dropping it
/// afterwards aborts that task, so later portal calls (for example from a
/// second `Enigo`) wait forever for a reply that can no longer arrive. A
/// single runtime is therefore kept alive for the whole process.
#[cfg(any(feature = "libei", feature = "xdg_desktop"))]
mod portal {
    use std::future::Future;

    /// Run `future` to completion.
    ///
    /// Inside an existing Tokio runtime the future is driven here instead of
    /// starting another runtime, because a runtime cannot be nested. On a
    /// multi-thread runtime, `block_in_place` hands the worker's other tasks to
    /// another thread while this one blocks, so the runtime keeps making
    /// progress.
    #[allow(clippy::unnecessary_wraps)] // The wrap is needed for the tokio feature
    pub(crate) fn block_on<F: Future>(future: F) -> Result<F::Output, std::io::Error> {
        #[cfg(feature = "tokio")]
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // A current-thread runtime has no other worker to hand tasks off
            // to, so `block_in_place` would panic. Drive the future directly;
            // its background tasks then have to live on the shared runtime.
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                // Build the shared runtime inside `block_in_place` so a losing
                // runtime in the initialization race is not dropped inside an
                // async context, where dropping blocks and would panic.
                return tokio::task::block_in_place(|| {
                    let runtime = runtime()?;
                    Ok(runtime.block_on(future))
                });
            }
            return Ok(futures::executor::block_on(future));
        }

        #[cfg(feature = "tokio")]
        return Ok(runtime()?.block_on(future));

        #[cfg(not(feature = "tokio"))]
        Ok(futures::executor::block_on(future))
    }

    /// The process-wide runtime used when the caller does not have a Tokio
    /// runtime of its own. It has to outlive every connection because
    /// `ashpd`/`zbus` keep the D-Bus reader task on the runtime that created
    /// the connection.
    ///
    /// This relies on the Tokio features that `ashpd/tokio` enables through
    /// `zbus/tokio` (`rt-multi-thread` for `new_multi_thread` and the I/O and
    /// time drivers for `enable_all`).
    #[cfg(feature = "tokio")]
    fn runtime() -> Result<&'static tokio::runtime::Runtime, std::io::Error> {
        use std::sync::OnceLock;

        static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
        if let Some(runtime) = RUNTIME.get() {
            return Ok(runtime);
        }

        let built = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        // If another thread initialized the runtime in the meantime it wins and
        // the one built here is dropped again. No tasks were spawned on it yet.
        let _ = RUNTIME.set(built);
        Ok(RUNTIME
            .get()
            .expect("the shared tokio runtime is initialized above"))
    }

    #[cfg(all(test, feature = "tokio"))]
    mod tests {
        use super::block_on;
        use std::{
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::Duration,
        };

        /// `ashpd`/`zbus` spawn the D-Bus reader task on the runtime that
        /// created the connection, so it has to survive the `block_on` call
        /// that created it. Dropping the runtime would cancel the task and
        /// make later portal calls hang.
        #[test]
        fn background_tasks_outlive_block_on() {
            let done = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&done);

            block_on(async move {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    flag.store(true, Ordering::SeqCst);
                });
            })
            .unwrap();

            for _ in 0..100 {
                if done.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("a task spawned on the shared runtime did not run after block_on returned");
        }

        /// A later portal call must still receive work produced by a task that
        /// was spawned during an earlier call. This models a second `Enigo` (or
        /// the next input method call) that used to hang because the cached
        /// `ashpd` connection's reader task was cancelled together with the
        /// runtime that created it.
        #[test]
        fn later_call_receives_result_from_task_of_earlier_call() {
            use tokio::sync::oneshot;

            let (sender, receiver) = oneshot::channel();

            // Models the first portal call (e.g. `Enigo::new`), which spawns
            // the connection's reader task and then returns.
            block_on(async move {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    let _ = sender.send(42);
                });
            })
            .unwrap();

            // Models a later call on the same connection. If the runtime had
            // been dropped after the first call, the spawned task would have
            // been cancelled, the sender dropped, and this would resolve to an
            // error instead of `42`.
            let value = block_on(receiver)
                .unwrap()
                .expect("the task spawned by the earlier call was cancelled");
            assert_eq!(value, 42);
        }

        /// When called from inside a multi-thread Tokio runtime, blocking must
        /// not starve that runtime: `block_in_place` hands its other tasks to a
        /// different thread.
        #[test]
        fn blocking_from_an_ambient_runtime_does_not_starve_it() {
            use std::{sync::mpsc, task::Poll, time::Instant};

            let ambient = tokio::runtime::Builder::new_multi_thread()
                // One worker so the blocking call takes the only one there is.
                .worker_threads(1)
                .enable_all()
                .build()
                .unwrap();

            let value = ambient.block_on(async {
                let (done_sender, done_receiver) = tokio::sync::oneshot::channel();

                // Block from inside an async task, like a user calling Enigo
                // from `#[tokio::main]`.
                tokio::spawn(async move {
                    let (sender, receiver) = mpsc::channel();
                    // Has to make progress on the ambient runtime while the
                    // worker is blocked.
                    tokio::spawn(async move {
                        let _ = sender.send(7);
                    });

                    let deadline = Instant::now() + Duration::from_secs(2);
                    // Poll with a deadline that does not depend on either
                    // runtime's timer, so a regression fails instead of
                    // hanging.
                    let value = block_on(futures::future::poll_fn(move |cx| {
                        match receiver.try_recv() {
                            Ok(value) => return Poll::Ready(Some(value)),
                            Err(mpsc::TryRecvError::Disconnected) => return Poll::Ready(None),
                            Err(mpsc::TryRecvError::Empty) => {}
                        }
                        if Instant::now() >= deadline {
                            return Poll::Ready(None);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }))
                    .unwrap();

                    let _ = done_sender.send(value);
                });

                done_receiver.await.expect("the blocking task was dropped")
            });

            assert_eq!(value, Some(7), "blocking starved the ambient runtime");
        }
    }
}

#[cfg(any(
    all(feature = "libei", feature = "tokio"),
    all(feature = "libei", feature = "smol")
))]
mod libei;

#[cfg(feature = "wayland")]
mod wayland;
#[cfg(any(feature = "x11rb", feature = "xdo"))]
#[cfg_attr(feature = "x11rb", path = "x11rb.rs")]
#[cfg_attr(not(feature = "x11rb"), path = "xdo.rs")]
mod x11;

#[cfg(feature = "xdg_desktop")]
mod xdg_desktop;

#[cfg(any(feature = "wayland", feature = "x11rb"))]
mod xkb;

#[cfg(any(feature = "wayland", feature = "libei"))]
mod text;

pub struct Enigo {
    held: (Vec<Key>, Vec<u16>), // Currently held keys and held keycodes
    release_keys_when_dropped: bool,
    #[cfg(feature = "wayland")]
    wayland: Option<wayland::Con>,
    #[cfg(any(feature = "x11rb", feature = "xdo"))]
    x11: Option<x11::Con>,
    #[cfg(any(
        all(feature = "libei", feature = "tokio"),
        all(feature = "libei", feature = "smol")
    ))]
    libei: Option<libei::Con>,
    #[cfg(any(
        all(feature = "xdg_desktop", feature = "tokio"),
        all(feature = "xdg_desktop", feature = "smol")
    ))]
    xdg_desktop: Option<xdg_desktop::Con>,
    #[cfg(not(any(
        all(feature = "xdg_desktop", feature = "tokio"),
        all(feature = "xdg_desktop", feature = "smol")
    )))]
    _phantom: std::marker::PhantomData<()>, /* Needed to fix compiler complaining about
                                             * unused lifetime
                                             * parameter */
}

impl Enigo {
    /// Create a new Enigo struct to establish the connection to simulate input
    /// with the specified settings
    ///
    /// # Errors
    /// Have a look at the documentation of `NewConError` to see under which
    /// conditions an error will be returned.
    pub fn new(settings: &Settings) -> Result<Self, NewConError> {
        let mut connection_established = false;
        #[allow(unused_variables)]
        let Settings {
            x11_display,
            wayland_display,
            release_keys_when_dropped,
            restore_token,
            ..
        } = settings;

        let held = (Vec::new(), Vec::new());

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        let xdg_desktop = match xdg_desktop::Con::new(restore_token.as_deref()) {
            Ok(con) => {
                connection_established = true;
                debug!("xdg_desktop connection established");
                Some(con)
            }
            Err(e) => {
                warn!("{e}");
                None
            }
        };
        #[cfg(feature = "wayland")]
        let wayland = match wayland::Con::new(wayland_display.as_deref()) {
            Ok(con) => {
                connection_established = true;
                debug!("wayland connection established");
                Some(con)
            }
            Err(e) => {
                warn!("{e}");
                None
            }
        };
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        match x11_display {
            Some(name) => {
                debug!("\x1b[93mtrying to establish a x11 connection to: {name}\x1b[0m");
            }
            None => {
                debug!("\x1b[93mtrying to establish a x11 connection to $DISPLAY\x1b[0m");
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        let x11 = match x11::Con::new(x11_display.as_deref()) {
            Ok(con) => {
                connection_established = true;
                debug!("x11 connection established");
                Some(con)
            }
            Err(e) => {
                warn!("failed to establish x11 connection: {e}");
                None
            }
        };
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        let libei = match libei::Con::new(restore_token.as_deref()) {
            Ok(con) => {
                connection_established = true;
                debug!("libei connection established");
                Some(con)
            }
            Err(e) => {
                warn!("failed to establish libei connection: {e}");
                None
            }
        };
        if !connection_established {
            error!("no successful connection");
            return Err(NewConError::EstablishCon("no successful connection"));
        }

        Ok(Self {
            held,
            release_keys_when_dropped: *release_keys_when_dropped,
            #[cfg(feature = "wayland")]
            wayland,
            #[cfg(any(feature = "x11rb", feature = "xdo"))]
            x11,
            #[cfg(any(
                all(feature = "libei", feature = "tokio"),
                all(feature = "libei", feature = "smol")
            ))]
            libei,
            #[cfg(any(
                all(feature = "xdg_desktop", feature = "tokio"),
                all(feature = "xdg_desktop", feature = "smol")
            ))]
            xdg_desktop,
            #[cfg(not(any(
                all(feature = "xdg_desktop", feature = "tokio"),
                all(feature = "xdg_desktop", feature = "smol")
            )))]
            _phantom: std::marker::PhantomData,
        })
    }

    /// Returns a list of all currently pressed keys
    pub fn held(&mut self) -> (Vec<Key>, Vec<u16>) {
        self.held.clone()
    }

    /// Returns the restore token from the portal session when using `libei` or
    /// `xdg_desktop`, if available. Save this token and pass it via
    /// `Settings::restore_token` on the next connection to avoid the
    /// permission dialog.
    #[must_use]
    #[cfg(feature = "platform_specific")]
    #[cfg_attr(docsrs, doc(cfg(feature = "platform_specific")))]
    pub fn restore_token(&self) -> Option<String> {
        #[cfg(feature = "xdg_desktop")]
        if let Some(token) = self
            .xdg_desktop
            .as_ref()
            .and_then(xdg_desktop::Con::restore_token)
        {
            return Some(token);
        }
        #[cfg(feature = "libei")]
        if let Some(token) = self.libei.as_ref().and_then(libei::Con::restore_token) {
            return Some(token);
        }
        None
    }
}

impl Mouse for Enigo {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mbutton(button: {button:?}, direction: {direction:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try sending button event via xdg_desktop");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try sending button event via wayland");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try sending button event via x11");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try sending button event via libei");
            res = con.button(button, direction);
            if res.is_ok() {
                debug!("successfully sent button event via libei");
                return res;
            }
        }
        res
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        debug!("\x1b[93mmove_mouse(x: {x:?}, y: {y:?}, coordinate:{coordinate:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));
        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try moving the mouse via xdg_desktop");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try moving the mouse via wayland");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try moving the mouse via x11");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try moving the mouse via libei");
            res = con.move_mouse(x, y, coordinate);
            if res.is_ok() {
                debug!("successfully moved the mouse via libei");
                return res;
            }
        }
        res
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93mscroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try scrolling via xdg_desktop");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try scrolling via wayland");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via wayland");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_mut() {
            trace!("try scrolling via x11");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try scrolling via libei");
            res = con.scroll(length, axis);
            if res.is_ok() {
                debug!("successfully scrolled via libei");
                return res;
            }
        }
        res
    }

    fn smooth_scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        debug!("\x1b[93msmooth_scroll(length: {length:?}, axis: {axis:?})\x1b[0m");
        // `mut` is unused when none of the protocols that support smooth scroll
        // are enabled.
        #[allow(unused_mut)]
        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try smooth scrolling via xdg_desktop");
            res = con.smooth_scroll(length, axis);
            if res.is_ok() {
                debug!("successfully smooth scrolled via xdg_desktop");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try smooth scrolling via wayland");
            res = con.smooth_scroll(length, axis);
            if res.is_ok() {
                debug!("successfully smooth scrolled via wayland");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try smooth scrolling via libei");
            res = con.smooth_scroll(length, axis);
            if res.is_ok() {
                debug!("successfully smooth scrolled via libei");
                return res;
            }
        }
        res
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mmain_display()\x1b[0m");
        let mut res = Err(InputError::Simulate(
            "No protocol to get the main display dimensions",
        ));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_ref() {
            trace!("try getting the dimensions of the display via xdg_desktop");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            trace!("try getting the dimensions of the display via wayland");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            trace!("try getting the dimensions of the display via x11");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_ref() {
            trace!("try getting the dimensions of the display via libei");
            res = con.main_display();
            if res.is_ok() {
                debug!("successfully got the dimensions");
                return res;
            }
        }
        res
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        debug!("\x1b[93mlocation()\x1b[0m");
        let mut res = Err(InputError::Simulate(
            "No protocol to get the mouse location",
        ));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_ref() {
            trace!("try getting the mouse location via xdg_desktop");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_ref() {
            trace!("try getting the mouse location via wayland");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if let Some(con) = self.x11.as_ref() {
            trace!("try getting the mouse location via x11");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_ref() {
            trace!("try getting the mouse location via libei");
            res = con.location();
            if res.is_ok() {
                debug!("successfully got the mouse location");
                return res;
            }
        }
        res
    }
}

impl Keyboard for Enigo {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        debug!("\x1b[93mfast_text(text: {text})\x1b[0m");
        #[allow(unused_mut)]
        let mut res = Ok(None); // Don't return an error here so it can be retried entering individual letters

        /*
        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering text fast via xdg_desktop");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via xdg_desktop");
                return res;
            }
        }*/
        #[cfg(feature = "wayland")]
        if let Some(con) = self.wayland.as_mut() {
            trace!("try entering text fast via wayland");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via wayland");
                return res;
            }
        }
        //#[cfg(any(feature = "x11rb", feature = "xdo"))] // Not possible on
        //#[cfg(any(feature x11rb
        #[cfg(feature = "xdo")]
        if let Some(con) = self.x11.as_mut() {
            trace!("try entering text fast via x11");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via x11");
                return res;
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if let Some(con) = self.libei.as_mut() {
            trace!("try entering text fast via libei");
            res = con.fast_text(text);
            if res.is_ok() {
                debug!("successfully entered text fast via libei");
                return res;
            }
        }
        res
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mkey(key: {key:?}, direction: {direction:?})\x1b[0m");
        // Nothing to do
        if key == Key::Unicode('\0') {
            debug!("entering the null byte is a noop");
            return Ok(());
        }

        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering the key via xdg_desktop");
            res = con.key(key, direction);
            if res.is_ok() {
                debug!("successfully entered the key via xdg_desktop");
            }
        }
        #[cfg(feature = "wayland")]
        if res.is_err() {
            if let Some(con) = self.wayland.as_mut() {
                trace!("try entering the key via wayland");
                res = con.key(key, direction);
                if res.is_ok() {
                    debug!("successfully entered the key via wayland");
                }
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if res.is_err() {
            if let Some(con) = self.x11.as_mut() {
                trace!("try entering the key via x11");
                res = con.key(key, direction);
                if res.is_ok() {
                    debug!("successfully entered the key via x11");
                }
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if res.is_err() {
            if let Some(con) = self.libei.as_mut() {
                trace!("try entering the key via libei");
                res = con.key(key, direction);
                if res.is_ok() {
                    debug!("successfully entered the key via libei");
                }
            }
        }

        // Record held keys only after a successful press/release
        if res.is_ok() {
            match direction {
                Direction::Press => {
                    debug!("added the key {key:?} to the held keys");
                    self.held.0.push(key);
                }
                Direction::Release => {
                    debug!("removed the key {key:?} from the held keys");
                    self.held.0.retain(|&k| k != key);
                }
                Direction::Click => (),
            }
        }

        res
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        debug!("\x1b[93mraw(keycode: {keycode:?}, direction: {direction:?})\x1b[0m");

        let mut res = Err(InputError::Simulate("No protocol to simulate the input"));

        #[cfg(any(
            all(feature = "xdg_desktop", feature = "tokio"),
            all(feature = "xdg_desktop", feature = "smol")
        ))]
        if let Some(con) = self.xdg_desktop.as_mut() {
            trace!("try entering the keycode via xdg_desktop");
            res = con.raw(keycode, direction);
            if res.is_ok() {
                debug!("successfully entered the raw key via xdg_desktop");
            }
        }
        #[cfg(feature = "wayland")]
        if res.is_err() {
            if let Some(con) = self.wayland.as_mut() {
                trace!("try entering the keycode via wayland");
                res = con.raw(keycode, direction);
                if res.is_ok() {
                    debug!("successfully entered the raw key via wayland");
                }
            }
        }
        #[cfg(any(feature = "x11rb", feature = "xdo"))]
        if res.is_err() {
            if let Some(con) = self.x11.as_mut() {
                trace!("try entering the keycode via x11");
                res = con.raw(keycode, direction);
                if res.is_ok() {
                    debug!("successfully entered the raw key via x11");
                }
            }
        }
        #[cfg(any(
            all(feature = "libei", feature = "tokio"),
            all(feature = "libei", feature = "smol")
        ))]
        if res.is_err() {
            if let Some(con) = self.libei.as_mut() {
                trace!("try entering the keycode via libei");
                res = con.raw(keycode, direction);
                if res.is_ok() {
                    debug!("successfully entered the raw key via libei");
                }
            }
        }

        // Record held keycodes only after a successful press/release
        if res.is_ok() {
            match direction {
                Direction::Press => {
                    debug!("added the keycode {keycode:?} to the held keys");
                    self.held.1.push(keycode);
                }
                Direction::Release => {
                    debug!("removed the keycode {keycode:?} from the held keys");
                    self.held.1.retain(|&k| k != keycode);
                }
                Direction::Click => (),
            }
        }

        res
    }
}

impl Drop for Enigo {
    // Release the held keys before the connection is dropped
    fn drop(&mut self) {
        if !self.release_keys_when_dropped {
            return;
        }
        let (held_keys, held_keycodes) = self.held();
        for &key in &held_keys {
            if self.key(key, Direction::Release).is_err() {
                error!("unable to release {key:?}");
            }
        }
        for &keycode in &held_keycodes {
            if self.raw(keycode, Direction::Release).is_err() {
                error!("unable to release {keycode:?}");
            }
        }
        debug!("released all held keys and held keycodes");
    }
}
