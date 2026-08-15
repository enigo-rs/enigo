use ashpd::desktop::{
    CreateSessionOptions, Session,
    remote_desktop::{
        DeviceType, KeyState, NotifyKeyboardKeycodeOptions, NotifyKeyboardKeysymOptions,
        NotifyPointerAxisDiscreteOptions, NotifyPointerAxisOptions, NotifyPointerButtonOptions,
        NotifyPointerMotionOptions, RemoteDesktop, SelectDevicesOptions, StartOptions,
    },
};
use log::{debug, error, trace, warn};

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

/// The main struct for handling the event emitting
pub struct Con {
    // Listed first so it is dropped last: the session still needs the runtime.
    runtime: PortalTokioRuntime,
    session: Session<RemoteDesktop>,
    remote_desktop: RemoteDesktop,
    #[cfg(feature = "platform_specific")]
    restore_token: Option<String>,
}

/// Owned Tokio runtime for portal I/O.
///
/// Uses `shutdown_background` on drop so `Con` can be dropped from inside
/// another Tokio runtime without panicking.
#[cfg(feature = "tokio")]
struct PortalTokioRuntime(Option<tokio::runtime::Runtime>);

#[cfg(not(feature = "tokio"))]
struct PortalTokioRuntime;

#[cfg(feature = "tokio")]
impl PortalTokioRuntime {
    fn new() -> Result<Self, NewConError> {
        Ok(Self(Some(
            tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .map_err(|e| {
                    error!("{e}");
                    NewConError::EstablishCon("failed to create tokio runtime")
                })?,
        )))
    }
}

#[cfg(not(feature = "tokio"))]
impl PortalTokioRuntime {
    fn new() -> Self {
        Self
    }
}

#[cfg(feature = "tokio")]
impl Drop for PortalTokioRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}

/// Drive a portal future to completion.
///
/// With `tokio`, reuses the owned runtime. When already inside another Tokio
/// runtime, `block_in_place` makes nested `block_on` safe on multi-thread
/// runtimes (the usual `#[tokio::main]` / CI case). With `smol`, uses
/// `futures::executor`.
fn block_on_portal<F: Future>(runtime: &PortalTokioRuntime, f: F) -> F::Output {
    #[cfg(feature = "tokio")]
    {
        let runtime = runtime.0.as_ref().expect("runtime shut down");
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| runtime.block_on(f))
        } else {
            runtime.block_on(f)
        }
    }
    #[cfg(not(feature = "tokio"))]
    {
        let _ = runtime;
        futures::executor::block_on(f)
    }
}

unsafe impl Send for Con {}

/// Whether the current session is Wayland, which is the only place the
/// `RemoteDesktop` portal is served.
fn is_wayland_session() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return true;
    }

    std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t.eq_ignore_ascii_case("wayland"))
}

impl Con {
    async fn open_connection(
        restore_token: Option<&str>,
    ) -> Result<(Session<RemoteDesktop>, RemoteDesktop, Option<String>), NewConError> {
        trace!("open_connection");

        // Compositors reject RemoteDesktop outright under X11, and some of them
        // (KWin) raise a modal error dialog for every attempt, which also steals
        // focus from whatever the caller was about to type into. Bail out before
        // asking so the x11 backend is reached silently.
        if !is_wayland_session() {
            debug!("not a wayland session, skipping the remote desktop portal");
            return Err(NewConError::EstablishCon(
                "the remote desktop portal is only available in wayland sessions",
            ));
        }

        let remote_desktop = RemoteDesktop::new().await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create RemoteDesktop")
        })?;
        trace!("New desktop");

        let session = remote_desktop
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to create remote desktop session")
            })?;

        let mut options = SelectDevicesOptions::default()
            .set_devices(DeviceType::Keyboard | DeviceType::Pointer)
            .set_persist_mode(ashpd::desktop::PersistMode::ExplicitlyRevoked);
        if let Some(restore_token) = restore_token {
            options = options.set_restore_token(restore_token);
        }

        remote_desktop
            .select_devices(&session, options)
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to select devices")
            })?;
        trace!("new session");

        let restore_token = remote_desktop
            .start(&session, None, StartOptions::default())
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to start remote desktop session")
            })?
            .response()
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to get remote desktop session response")
            })?
            .restore_token()
            .map(str::to_owned);
        trace!("start session");
        Ok((session, remote_desktop, restore_token))
    }

    /// Create a new Enigo instance
    pub fn new(restore_token: Option<&str>) -> Result<Self, NewConError> {
        debug!("using xdg desktop");

        #[cfg(feature = "tokio")]
        let runtime = PortalTokioRuntime::new()?;
        #[cfg(not(feature = "tokio"))]
        let runtime = PortalTokioRuntime::new();
        let (session, remote_desktop, restore_token) =
            block_on_portal(&runtime, Self::open_connection(restore_token))?;

        #[cfg(not(feature = "platform_specific"))]
        let _ = restore_token;
        Ok(Self {
            runtime,
            session,
            remote_desktop,
            #[cfg(feature = "platform_specific")]
            restore_token,
        })
    }

    /// Returns the restore token from the portal session, if one was issued.
    /// Callers should save this token and pass it via `Settings::restore_token`
    /// on the next connection to skip the permission dialog.
    #[must_use]
    #[cfg(feature = "platform_specific")]
    pub fn restore_token(&self) -> Option<String> {
        self.restore_token.clone()
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, _text: &str) -> InputResult<Option<()>> {
        warn!("fast text entry is not yet implemented with xdg_desktop");
        // TODO: Add fast method
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let keysym = xkeysym::Keysym::from(key).raw().try_into().map_err(|_| {
            log::error!("The keysym was larger than i32::MAX. This should never happen");
            InputError::InvalidInput("The keysym was larger than i32::MAX")
        })?;

        let key_states = match direction {
            Direction::Press => vec![KeyState::Pressed],
            Direction::Release => vec![KeyState::Released],
            Direction::Click => vec![KeyState::Pressed, KeyState::Released],
        };

        for key_state in key_states {
            block_on_portal(
                &self.runtime,
                self.remote_desktop.notify_keyboard_keysym(
                    &self.session,
                    keysym,
                    key_state,
                    NotifyKeyboardKeysymOptions::default(),
                ),
            )
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed to send keysym")
            })?;
        }

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        // Public API uses X11 keycodes (same as the libei/wayland backends).
        // NotifyKeyboardKeycode expects Linux evdev keycodes (X11 − 8).
        let keycode = i32::from(keycode).checked_sub(8).ok_or({
            InputError::InvalidInput("the keycode must be at least 8 (X11 keycode offset)")
        })?;

        let key_states = match direction {
            Direction::Press => vec![KeyState::Pressed],
            Direction::Release => vec![KeyState::Released],
            Direction::Click => vec![KeyState::Pressed, KeyState::Released],
        };

        for key_state in key_states {
            block_on_portal(
                &self.runtime,
                self.remote_desktop.notify_keyboard_keycode(
                    &self.session,
                    keycode,
                    key_state,
                    NotifyKeyboardKeycodeOptions::default(),
                ),
            )
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed to send keycode")
            })?;
        }

        Ok(())
    }
}

impl Mouse for Con {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        // Releasing a scroll "button" has no effect
        if direction == Direction::Release {
            match button {
                Button::ScrollDown
                | Button::ScrollUp
                | Button::ScrollRight
                | Button::ScrollLeft => return Ok(()),
                Button::Left | Button::Right | Button::Back | Button::Forward | Button::Middle => {}
            }
        }

        let code = match button {
            // Taken from /linux/input-event-codes.h
            Button::Left => 0x110,
            Button::Right => 0x111,
            Button::Back => 0x116,
            Button::Forward => 0x115,
            Button::Middle => 0x112,
            Button::ScrollDown => return self.scroll(1, Axis::Vertical),
            Button::ScrollUp => return self.scroll(-1, Axis::Vertical),
            Button::ScrollRight => return self.scroll(1, Axis::Horizontal),
            Button::ScrollLeft => return self.scroll(-1, Axis::Horizontal),
        };

        let key_states = match direction {
            Direction::Press => vec![KeyState::Pressed],
            Direction::Release => vec![KeyState::Released],
            Direction::Click => vec![KeyState::Pressed, KeyState::Released],
        };

        for key_state in key_states {
            block_on_portal(
                &self.runtime,
                self.remote_desktop.notify_pointer_button(
                    &self.session,
                    code,
                    key_state,
                    NotifyPointerButtonOptions::default(),
                ),
            )
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed to notify pointer button")
            })?;
        }

        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        match coordinate {
            Coordinate::Abs => {
                /*
                TODO: Implement this
                block_on_portal(&self.runtime, self.remote_desktop.notify_pointer_motion_absolute(
                    &self.session,
                    0, // TODO: Check which value is correct here
                    x as f64,
                    y as f64,
                ))
                .map_err(|e| {
                    log::error!("{e}");
                    InputError::Simulate("Failed to notify pointer motion absolute")
                })?;
                */

                // Stupid hack to circumvent the limitation of the portal. You cannot move the
                // mouse to an absolute coordinate without starting a screen cast
                self.move_mouse(i32::MIN, i32::MIN, Coordinate::Rel)?;
                self.move_mouse(x, y, Coordinate::Rel)
            }
            Coordinate::Rel => block_on_portal(
                &self.runtime,
                self.remote_desktop.notify_pointer_motion(
                    &self.session,
                    x as f64,
                    y as f64,
                    NotifyPointerMotionOptions::default(),
                ),
            )
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed to notify pointer motion relative")
            }),
        }
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let axis = match axis {
            Axis::Horizontal => ashpd::desktop::remote_desktop::Axis::Horizontal,
            Axis::Vertical => ashpd::desktop::remote_desktop::Axis::Vertical,
        };

        block_on_portal(
            &self.runtime,
            self.remote_desktop.notify_pointer_axis_discrete(
                &self.session,
                axis,
                length,
                NotifyPointerAxisDiscreteOptions::default(),
            ),
        )
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed to scroll")
        })?;

        Ok(())
    }

    fn smooth_scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let (dx, dy) = match axis {
            Axis::Horizontal => (f64::from(length), 0.0),
            Axis::Vertical => (0.0, f64::from(length)),
        };

        block_on_portal(
            &self.runtime,
            self.remote_desktop.notify_pointer_axis(
                &self.session,
                dx,
                dy,
                // One-shot smooth scroll: mark the sequence finished so compositors
                // don't keep waiting for further axis events / kinetic scroll.
                NotifyPointerAxisOptions::default().set_finish(true),
            ),
        )
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed to smooth scroll")
        })?;

        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        let (width, height) = block_on_portal(&self.runtime, async {
            let response = ashpd::desktop::screenshot::Screenshot::request()
                .interactive(false)
                .modal(true) // I don't see a modal and it prevents a scary warning
                .send()
                .await
                .map_err(|_| InputError::Simulate("Screenshot request failed"))?
                .response()
                .map_err(|_| InputError::Simulate("Screenshot response failed"))?;

            // Expect file:// URI
            let path = response.uri().as_str();

            let img = image::open(path)
                .map_err(|_| InputError::Simulate("Failed to open screenshot image"))?;
            if std::fs::remove_file(path).is_err() {
                log::error!(
                    "error deleting the temporary screenshot to get the dimensions of the screen"
                );
            }

            let (x, y) = (img.width() as i32, img.height() as i32);

            Ok((x, y))
        })
        .map_err(|e: InputError| {
            log::error!("{e}");
            InputError::Simulate("Failed to scroll")
        })?;

        Ok((width, height))
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        error!(
            "You tried to get the mouse location. I don't think that is possible with xdg_desktop"
        );
        Err(InputError::Simulate("Not possible with this protocol"))
    }
}

/// Does not need a portal — only the owned-runtime / nested-Tokio path.
#[cfg(all(test, feature = "tokio"))]
mod tests {
    use super::{PortalTokioRuntime, block_on_portal};
    use std::time::Duration;

    #[test]
    fn unit_portal_tokio_from_async_does_not_hang() {
        let outer = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        outer.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                let runtime = PortalTokioRuntime::new().unwrap();
                // Same pattern as press then release: two awaits on one runtime.
                assert_eq!(block_on_portal(&runtime, async { 1 }), 1);
                assert_eq!(block_on_portal(&runtime, async { 2 }), 2);
                // Drop while still inside the outer runtime
                // (shutdown_background).
            })
            .await
            .expect("nested portal runtime stalled when called from async code");
        });
    }
}
