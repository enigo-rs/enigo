use ashpd::desktop::{
    Session,
    remote_desktop::{KeyState, RemoteDesktop},
};
use log::{debug, error, trace, warn};

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};

/// The main struct for handling the event emitting
pub struct Con<'a> {
    session: Session<'a, RemoteDesktop<'a>>,
    remote_desktop: RemoteDesktop<'a>,
}

unsafe impl Send for Con<'_> {}

impl Con<'_> {
    async fn open_connection<'a>()
    -> Result<(Session<'a, RemoteDesktop<'a>>, RemoteDesktop<'a>), NewConError> {
        use ashpd::desktop::remote_desktop::DeviceType;

        trace!("open_connection");

        // Fallback: use portal
        let remote_desktop = RemoteDesktop::new().await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create RemoteDesktop")
        })?;
        trace!("New desktop");

        let session = remote_desktop.create_session().await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create remote desktop session")
        })?;

        remote_desktop
            .select_devices(
                &session,
                // TODO: Add DeviceType::Touchscreen once we support it in enigo
                DeviceType::Keyboard | DeviceType::Pointer,
                None, // TODO: Allow passing the restore_token via the EnigoSettings
                ashpd::desktop::PersistMode::Application, /* TODO: Allow passing the
                       * restore_token via the
                       * EnigoSettings */
            )
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to select devices")
            })?;
        trace!("new session");

        remote_desktop.start(&session, None).await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to start remote desktop session")
        })?;
        trace!("start session");
        Ok((session, remote_desktop))
    }

    #[allow(unnecessary_wraps)]
    fn custom_block_on<F>(f: F) -> Result<F::Output, std::io::Error>
    where
        F: Future,
    {
        #[cfg(feature = "tokio")]
        if tokio::runtime::Handle::try_current().is_err() {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()?; // return the error directly
            return Ok(rt.block_on(f));
        }

        Ok(futures::executor::block_on(f))
    }

    #[allow(clippy::unnecessary_wraps)]
    /// Create a new Enigo instance
    pub fn new() -> Result<Self, NewConError> {
        debug!("using xdg desktop");
        let (session, remote_desktop) =
            Self::custom_block_on(Self::open_connection()).map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to create tokio runtime")
            })??;
        Ok(Self {
            session,
            remote_desktop,
        })
    }
}

impl Keyboard for Con<'_> {
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
        let key_states = match direction {
            Direction::Press => vec![KeyState::Pressed],
            Direction::Release => vec![KeyState::Released],
            Direction::Click => vec![KeyState::Pressed, KeyState::Released],
        };

        for key_state in key_states {
            Self::custom_block_on(self.remote_desktop.notify_keyboard_keycode(
                &self.session,
                keycode.into(),
                key_state,
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

impl Mouse for Con<'_> {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
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

                // Stupid hack to circumvent the limitation of the portal. You cannot move the
                // mouse to an absolute coordinate without starting a screen cast
                self.move_mouse(i32::MIN, i32::MIN, Coordinate::Rel)?;
                self.move_mouse(x, y, Coordinate::Rel)
            }
            Coordinate::Rel => Self::custom_block_on(self.remote_desktop.notify_pointer_motion(
                &self.session,
                x as f64,
                y as f64,
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

    fn main_display(&self) -> InputResult<(i32, i32)> {
        error!(
            "You tried to get the main display. I don't think that is possible with xdg_desktop"
        );
        Err(InputError::Simulate("Not possible with this protocol"))
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        error!(
            "You tried to get the mouse location. I don't think that is possible with xdg_desktop"
        );
        Err(InputError::Simulate("Not possible with this protocol"))
    }
}
