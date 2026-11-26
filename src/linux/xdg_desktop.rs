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
    session: Session<RemoteDesktop>,
    remote_desktop: RemoteDesktop,
    #[cfg(feature = "platform_specific")]
    restore_token: Option<String>,
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

/// Convert an X11 keycode from the public `raw` API to the evdev keycode the
/// portal expects. The two are offset by 8; subtracting from a keycode below
/// the offset would underflow.
fn evdev_keycode(keycode: u16) -> InputResult<i32> {
    keycode
        .checked_sub(8)
        .map(i32::from)
        .ok_or(InputError::InvalidInput(
            "the keycode must be at least 8 (X11 keycode offset)",
        ))
}

/// Convert the `file://` URI returned by the Screenshot portal to a local
/// filesystem path. The URI is not a path, so passing it to `image::open` or
/// `std::fs::remove_file` fails even though the screenshot was written.
fn screenshot_path(uri: &ashpd::Uri) -> InputResult<std::path::PathBuf> {
    let url = url::Url::parse(uri.as_str()).map_err(|e| {
        error!("the screenshot portal returned an invalid URI: {e}");
        InputError::Simulate("the screenshot portal returned an invalid URI")
    })?;

    url.to_file_path().map_err(|()| {
        error!("the screenshot portal returned a URI that is not a local file: {uri}");
        InputError::Simulate("the screenshot portal returned a URI that is not a local file")
    })
}

impl Con {
    async fn open_connection(
        restore_token: Option<&str>,
    ) -> Result<(Session<RemoteDesktop>, RemoteDesktop, Option<String>), NewConError> {
        trace!("open_connection");

        // Compositors reject RemoteDesktop outright under X11, and some of them
        // (KWin) raise a modal error dialog for every attempt, which also
        // steals focus from whatever the caller was about to type into. Bail
        // out before asking so the x11 backend is reached silently.
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

        // An old portal cannot use the restore token. Keep it out of the
        // request and let the portal ask for permission again.
        let restore_token = if restore_token.is_some()
            && remote_desktop.version() < super::MIN_REMOTE_DESKTOP_VERSION
        {
            warn!(
                "the portal does not support restore tokens (RemoteDesktop version {}); \
                 asking for permission again. xdg-desktop-portal 1.20 or newer is required",
                remote_desktop.version()
            );
            None
        } else {
            restore_token
        };

        let session = remote_desktop
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to create remote desktop session")
            })?;

        let mut options = SelectDevicesOptions::default()
            .set_devices(DeviceType::KEYBOARD | DeviceType::POINTER)
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

        Self::pre_create_devices(&session, &remote_desktop).await;

        Ok((session, remote_desktop, restore_token))
    }

    /// Create the virtual keyboard and pointer up front.
    ///
    /// The portal creates each device lazily on the first event for its type,
    /// which is also the step that advertises the capability. A client that has
    /// not bound `wl_keyboard`/`wl_pointer` yet therefore misses that first
    /// event. Creating the devices here moves the capability advertisement
    /// before the caller's first real event.
    ///
    /// A relative motion of (0, 0) still creates the pointer device (Mutter
    /// emits a harmless empty motion). `NoSymbol` (0) creates the keyboard
    /// device; the compositor cannot map it and drops the event after logging
    /// "No keycode found for keyval 0", before it records any key state, so no
    /// key reaches an application and no release is needed.
    ///
    /// This is best effort: if the portal rejects a no-op, the devices stay
    /// lazy and nothing else changes.
    async fn pre_create_devices(session: &Session<RemoteDesktop>, remote_desktop: &RemoteDesktop) {
        if let Err(e) = remote_desktop
            .notify_pointer_motion(session, 0.0, 0.0, NotifyPointerMotionOptions::default())
            .await
        {
            debug!("failed to pre-create the virtual pointer: {e}");
        }

        if let Err(e) = remote_desktop
            .notify_keyboard_keysym(
                session,
                0,
                KeyState::Pressed,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await
        {
            debug!("failed to pre-create the virtual keyboard: {e}");
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn custom_block_on<F>(f: F) -> Result<F::Output, std::io::Error>
    where
        F: Future,
    {
        super::portal::block_on(f)
    }

    #[allow(clippy::unnecessary_wraps)]
    /// Create a new Enigo instance
    pub fn new(restore_token: Option<&str>) -> Result<Self, NewConError> {
        debug!("using xdg desktop");
        let (session, remote_desktop, restore_token) =
            Self::custom_block_on(Self::open_connection(restore_token)).map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to create tokio runtime")
            })??;
        #[cfg(not(feature = "platform_specific"))]
        let _ = restore_token;
        Ok(Self {
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
            Self::custom_block_on(self.remote_desktop.notify_keyboard_keysym(
                &self.session,
                keysym,
                key_state,
                NotifyKeyboardKeysymOptions::default(),
            ))
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed in custom_block_on")
            })?
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed to send keysym")
            })?;
        }

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        // The public API uses X11 keycodes like the other Linux backends.
        let keycode = evdev_keycode(keycode)?;

        let key_states = match direction {
            Direction::Press => vec![KeyState::Pressed],
            Direction::Release => vec![KeyState::Released],
            Direction::Click => vec![KeyState::Pressed, KeyState::Released],
        };

        for key_state in key_states {
            Self::custom_block_on(self.remote_desktop.notify_keyboard_keycode(
                &self.session,
                keycode,
                key_state,
                NotifyKeyboardKeycodeOptions::default(),
            ))
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed in custom_block_on")
            })?
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
            Self::custom_block_on(self.remote_desktop.notify_pointer_button(
                &self.session,
                code,
                key_state,
                NotifyPointerButtonOptions::default(),
            ))
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed in custom_block_on")
            })?
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
                Self::custom_block_on(self.remote_desktop.notify_pointer_motion_absolute(
                    &self.session,
                    0, // TODO: Check which value is correct here
                    x as f64,
                    y as f64,
                ))
                .map_err(|e| {
                    log::error!("{e}");
                    InputError::Simulate("Failed in custom_block_on")
                })?
                .map_err(|e| {
                    log::error!("{e}");
                    InputError::Simulate("Failed to notify pointer motion absolute")
                })?;
                */

                // Stupid hack to circumvent the limitation of the portal. You
                // cannot move the mouse to an absolute coordinate without
                // starting a screen cast
                self.move_mouse(i32::MIN, i32::MIN, Coordinate::Rel)?;
                self.move_mouse(x, y, Coordinate::Rel)
            }
            Coordinate::Rel => Self::custom_block_on(self.remote_desktop.notify_pointer_motion(
                &self.session,
                x as f64,
                y as f64,
                NotifyPointerMotionOptions::default(),
            ))
            .map_err(|e| {
                log::error!("{e}");
                InputError::Simulate("Failed in custom_block_on")
            })?
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

        Self::custom_block_on(self.remote_desktop.notify_pointer_axis_discrete(
            &self.session,
            axis,
            length,
            NotifyPointerAxisDiscreteOptions::default(),
        ))
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed in custom_block_on")
        })?
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

        Self::custom_block_on(self.remote_desktop.notify_pointer_axis(
            &self.session,
            dx,
            dy,
            // One-shot smooth scroll: mark the sequence finished so compositors
            // don't keep waiting for further axis events / kinetic scroll.
            NotifyPointerAxisOptions::default().set_finish(true),
        ))
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed in custom_block_on")
        })?
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed to smooth scroll")
        })?;

        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        Self::custom_block_on(async {
            let response = ashpd::desktop::screenshot::Screenshot::request()
                .interactive(false)
                .modal(true) // I don't see a modal and it prevents a scary warning
                .send()
                .await
                .map_err(|_| InputError::Simulate("Screenshot request failed"))?
                .response()
                .map_err(|_| InputError::Simulate("Screenshot response failed"))?;

            let path = screenshot_path(response.uri())?;

            let img = image::open(&path)
                .map_err(|_| InputError::Simulate("Failed to open screenshot image"))?;
            if std::fs::remove_file(&path).is_err() {
                log::error!(
                    "error deleting the temporary screenshot to get the dimensions of the screen"
                );
            }

            let (x, y) = (img.width() as i32, img.height() as i32);

            Ok((x, y))
        })
        .map_err(|e| {
            log::error!("{e}");
            InputError::Simulate("Failed in custom_block_on")
        })?
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        error!(
            "You tried to get the mouse location. I don't think that is possible with xdg_desktop"
        );
        Err(InputError::Simulate("Not possible with this protocol"))
    }
}

#[cfg(test)]
mod tests {
    use super::{evdev_keycode, screenshot_path};
    use crate::InputError;
    use std::path::PathBuf;

    #[test]
    fn converts_x11_keycodes_to_evdev() {
        assert_eq!(evdev_keycode(8), Ok(0));
        assert_eq!(evdev_keycode(9), Ok(1));
        assert_eq!(evdev_keycode(255), Ok(247));
    }

    #[test]
    fn rejects_keycodes_below_the_x11_offset() {
        assert!(matches!(evdev_keycode(7), Err(InputError::InvalidInput(_))));
    }

    #[test]
    fn converts_a_file_uri_to_a_path() {
        let uri = ashpd::Uri::parse("file:///tmp/screenshot.png").unwrap();
        assert_eq!(
            screenshot_path(&uri),
            Ok(PathBuf::from("/tmp/screenshot.png"))
        );
    }

    #[test]
    fn percent_decodes_the_path() {
        let uri = ashpd::Uri::parse("file:///tmp/a%20screenshot.png").unwrap();
        assert_eq!(
            screenshot_path(&uri),
            Ok(PathBuf::from("/tmp/a screenshot.png"))
        );
    }

    #[test]
    fn rejects_a_remote_uri() {
        let uri = ashpd::Uri::parse("https://example.com/screenshot.png").unwrap();
        assert!(matches!(
            screenshot_path(&uri),
            Err(InputError::Simulate(_))
        ));
    }
}
