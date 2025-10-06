use std::net::{TcpListener, TcpStream};

use tungstenite::accept;

use enigo::{Axis, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

use super::browser_events::{BrowserEvent, Event};

pub struct EnigoTest {
    enigo: Enigo,
    websocket: tungstenite::WebSocket<TcpStream>,
}

impl EnigoTest {
    pub fn new(settings: &Settings) -> Self {
        env_logger::try_init().ok();
        let enigo = Enigo::new(settings).expect("failed to create new enigo struct");
        let _ = &*super::browser::BROWSER_INSTANCE; // Launch Firefox
        let websocket = Self::websocket();

        websocket
            .get_ref()
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("Unable to set a read timeout");

        std::thread::sleep(std::time::Duration::from_secs(10)); // Give Firefox some time to launch
        Self { enigo, websocket }
    }

    fn websocket() -> tungstenite::WebSocket<TcpStream> {
        let listener = TcpListener::bind("127.0.0.1:26541").expect("failed to bind to port");
        log::debug!("TcpListener was created");
        let (stream, addr) = listener.accept().expect("Unable to accept the connection");
        log::debug!("New connection was made from {addr:?}");
        let websocket = accept(stream).expect("Unable to accept connections on the websocket");
        log::debug!("WebSocket was successfully created");
        websocket
    }

    fn read_message(&mut self) -> BrowserEvent {
        use crate::common::browser_events::BrowserEventError;

        log::debug!("Waiting for message on Websocket");
        let message = self
            .websocket
            .read()
            .expect("failed to read from websocket");
        log::debug!("Processing message");

        BrowserEvent::try_from(message).unwrap_or_else(|e| match e {
            BrowserEventError::WebsocketClosed => {
                panic!("Received a Close event")
            }
            _ => panic!("Other text received"),
        })
    }
}

/// Make sure the message queue is empty and all messages were processed
impl Drop for EnigoTest {
    fn drop(&mut self) {
        if self.websocket.read().is_ok() {
            panic!("there were messages left. This should never happen")
        }
    }
}

impl Keyboard for EnigoTest {
    fn fast_text(&mut self, text: &str) -> enigo::InputResult<Option<()>> {
        let mut expected_text = text.to_string();
        self.enigo.text(text).expect("failed to simulate text()");

        if cfg!(target_os = "macos") {}

        while !expected_text.is_empty() {
            let browser_event = self.read_message();
            let BrowserEvent {
                text: observed_text,
                event: Event::Key { key, .. },
            } = browser_event
            else {
                panic!("wrong event received: {browser_event:?}")
            };

            #[cfg(target_os = "macos")]
            let observed_text = key;

            match expected_text.strip_prefix(&observed_text) {
                Some(remainder) => expected_text = remainder.to_string(),
                None => panic!("failed to simulate text()"),
            }
        }

        Ok(Some(()))
    }

    fn key(&mut self, key: Key, direction: Direction) -> enigo::InputResult<()> {
        let expected_key = key;
        let expected_directions = match direction {
            Direction::Press => vec![Direction::Press],
            Direction::Release => vec![Direction::Release],
            Direction::Click => vec![Direction::Press, Direction::Release],
        };
        self.enigo
            .key(key, direction)
            .expect("failed to simulate key()");

        // The browser will send a press and release event for Direction::Click, so in
        // that case we need to make sure we received two correct events
        for expected_direction in expected_directions {
            let event = self.read_message().event;
            let Event::Key {
                timestamp,
                key,
                code,
                key_code,
                alt_key,
                ctrl_key,
                shift_key,
                meta_key,
                direction,
            } = event
            else {
                panic!("wrong event received: {event:?}")
            };
            let key = ron::from_str(&code)
                // TODO: Check if this is a good idea
                // It is done because on Windows, "code" is empty for the "Help" key. The "key"
                // field does contain the correct value though. We cannot always use the "key"
                // field, because it would make it impossible to differentiate left and right keys
                // (e.g LControl from RControl)
                .or_else(|_| ron::from_str(&key))
                .expect("failed to deserialize key");

            let keys_equal = matches!(
                (expected_key, key),
                (Key::Control, Key::LControl)
                    | (Key::LControl, Key::Control)
                    | (Key::Shift, Key::LShift)
                    | (Key::LShift, Key::Shift)
            ) || expected_key == key;

            assert!(keys_equal);
            assert_eq!(expected_direction, direction);
        }

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: enigo::Direction) -> enigo::InputResult<()> {
        let expected_keycode = keycode;
        let expected_directions = match direction {
            Direction::Press => vec![Direction::Press],
            Direction::Release => vec![Direction::Release],
            Direction::Click => vec![Direction::Press, Direction::Release],
        };
        self.enigo
            .raw(keycode, direction)
            .expect("failed to simulate raw()");
        // The browser will send a press and release event for Direction::Click, so in
        // that case we need to make sure we received two correct events
        for expected_direction in expected_directions {
            let event = self.read_message().event;
            let Event::Key {
                timestamp,
                key,
                code,
                key_code,
                alt_key,
                ctrl_key,
                shift_key,
                meta_key,
                direction,
            } = event
            else {
                panic!("wrong event received: {event:?}")
            };
            assert_eq!((expected_keycode, expected_direction), (keycode, direction));
        }
        Ok(())
    }
}

impl Mouse for EnigoTest {
    fn button(&mut self, button: enigo::Button, direction: Direction) -> enigo::InputResult<()> {
        let expected_button = button as u8;
        let expected_directions = match direction {
            Direction::Press => vec![Direction::Press],
            Direction::Release => vec![Direction::Release],
            Direction::Click => vec![Direction::Press, Direction::Release],
        };
        self.enigo
            .button(button, direction)
            .expect("failed to simulate button()");
        // The browser will send a press and release event for Direction::Click, so in
        // that case we need to make sure we received two correct events
        for expected_direction in expected_directions {
            let event = self.read_message().event;
            let Event::Button {
                timestamp,
                button,
                buttons,
                client_x,
                client_y,
                screen_x,
                screen_y,
                direction,
            } = event
            else {
                panic!("wrong event received: {event:?}")
            };

            assert_eq!((expected_button, expected_direction), (button, direction));
        }
        Ok(())
    }

    // Edge cases don't work (mouse is at the left most border and can't move one to
    // the left)
    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> enigo::InputResult<()> {
        self.enigo
            .move_mouse(x, y, coordinate)
            .expect("failed to simulate move_mouse()");
        let event = self.read_message().event;
        let Event::MouseMove {
            timestamp,
            client_x,
            client_y,
            movement_x,
            movement_y,
        } = event
        else {
            panic!("wrong event received: {event:?}")
        };
        match coordinate {
            Coordinate::Abs => assert_eq!((x, y), (client_x, client_y)),
            Coordinate::Rel => assert_eq!((x, y), (movement_x, movement_y)),
        }

        Ok(())
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> enigo::InputResult<()> {
        self.enigo
            .scroll(length, axis)
            .expect("failed to simulate scroll()");
        let event = self.read_message().event;
        let Event::Scroll {
            timestamp,
            delta_x,
            delta_y,
            delta_mode,
            client_x,
            client_y,
        } = event
        else {
            panic!("wrong event received: {event:?}")
        };
        let delta = match axis {
            Axis::Horizontal => delta_x,
            Axis::Vertical => delta_y,
        };
        assert_eq!(length as f64, delta);
        Ok(())
    }

    fn main_display(&self) -> enigo::InputResult<(i32, i32)> {
        let enigo_res = self
            .enigo
            .main_display()
            .expect("failed to get dimensions of the main display");
        let rdev_res = rdev_main_display();
        assert_eq!(
            enigo_res, rdev_res,
            "enigo_res: {enigo_res:?}; rdev_res: {rdev_res:?}"
        );
        Ok(enigo_res)
    }

    fn location(&self) -> enigo::InputResult<(i32, i32)> {
        let enigo_res = self
            .enigo
            .location()
            .expect("failed to get location of the mouse");
        let mouse_position_res = mouse_position();
        assert_eq!(
            enigo_res, mouse_position_res,
            "enigo_res: {enigo_res:?}; rdev_res: {mouse_position_res:?}"
        );
        Ok(enigo_res)
    }
}

fn rdev_main_display() -> (i32, i32) {
    rdev::display_size()
        .map(|(x, y)| (x as i32, y as i32))
        .expect("failed to get the location of the mouse using rdev")
}

fn mouse_position() -> (i32, i32) {
    use mouse_position::mouse_position::Mouse;

    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        _ => panic!("Unable to get the mouse position"),
    }
}
