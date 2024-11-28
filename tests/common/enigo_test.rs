use std::net::{TcpListener, TcpStream};

use tungstenite::accept;

use enigo::{
    Axis, Coordinate,
    Direction::{self, Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};

#[cfg(all(feature = "test_mouse", target_os = "windows"))]
use enigo::test_mouse::TestMouse;

use super::browser_events::BrowserEvent;

const TIMEOUT: u64 = 5; // Number of minutes the test is allowed to run before timing out
                        // This is needed, because some of the websocket functions are blocking and
                        // would run indefinitely without a timeout if they don't receive a message
const INPUT_DELAY: u64 = 40; // Number of milliseconds to wait for the input to have an effect
const SCROLL_STEP: (i32, i32) = (20, 114); // (horizontal, vertical)

pub struct EnigoTest {
    enigo: Enigo,
    websocket: tungstenite::WebSocket<TcpStream>,
    #[cfg(all(feature = "test_mouse", target_os = "windows"))]
    test_mouse: Option<TestMouse>,
}

impl EnigoTest {
    pub fn new(settings: &Settings) -> Self {
        env_logger::try_init().ok();
        EnigoTest::start_timeout_thread();
        let enigo = Enigo::new(settings).unwrap();

        let _ = &*super::browser::BROWSER_INSTANCE; // Launch Firefox
        let websocket = Self::websocket();

        #[cfg(all(feature = "test_mouse", target_os = "windows"))]
        let test_mouse = {
            let start_mouse = enigo.location().unwrap();
            let x = start_mouse.0;
            let y = start_mouse.1;
            let ballistic = settings.windows_subject_to_mouse_speed_and_acceleration_level;

            let test_mouse = TestMouse::new_simple(ballistic, x, y);

            Some(test_mouse)
        };

        std::thread::sleep(std::time::Duration::from_secs(10)); // Give Firefox some time to launch
        Self {
            enigo,
            websocket,
            #[cfg(all(feature = "test_mouse", target_os = "windows"))]
            test_mouse,
        }
    }

    fn websocket() -> tungstenite::WebSocket<TcpStream> {
        let listener = TcpListener::bind("127.0.0.1:26541").unwrap();
        println!("TcpListener was created");
        let (stream, addr) = listener.accept().expect("Unable to accept the connection");
        println!("New connection was made from {addr:?}");
        let websocket = accept(stream).expect("Unable to accept connections on the websocket");
        println!("WebSocket was successfully created");
        websocket
    }

    fn send_message(&mut self, msg: &str) {
        println!("Sending message: {msg}");
        self.websocket
            .send(tungstenite::Message::Text(msg.to_string()))
            .expect("Unable to send the message");
        println!("Sent message");
    }

    fn read_message(&mut self) -> BrowserEvent {
        println!("Waiting for message on Websocket");
        let message = self.websocket.read().unwrap();
        println!("Processing message");

        let Ok(browser_event) = BrowserEvent::try_from(message) else {
            panic!("Other text received");
        };
        assert!(
            !(browser_event == BrowserEvent::Close),
            "Received a Close event"
        );
        browser_event
    }

    fn start_timeout_thread() {
        // Spawn a thread to handle the timeout
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(TIMEOUT * 60));
            println!("Test suite exceeded the maximum allowed time of {TIMEOUT} minutes.");
            std::process::exit(1); // Exit with error code
        });
    }
}

impl Keyboard for EnigoTest {
    // This does not work for all text or the library does not work properly
    fn fast_text(&mut self, text: &str) -> enigo::InputResult<Option<()>> {
        self.send_message("ClearText");
        println!("Attempt to clear the text");
        assert_eq!(
            BrowserEvent::ReadyForText,
            self.read_message(),
            "Failed to get ready for the text"
        );
        let res = self.enigo.text(text);
        std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect
        self.send_message("GetText");

        let ev = self.read_message();
        if let BrowserEvent::Text(received_text) = ev {
            println!("received text: {received_text}");
            assert_eq!(text, received_text);
        } else {
            panic!("BrowserEvent was not a Text: {ev:?}");
        }

        res.map(Some) // TODO: Check if this is always correct
    }

    fn key(&mut self, key: Key, direction: Direction) -> enigo::InputResult<()> {
        let res = self.enigo.key(key, direction);
        if direction == Press || direction == Click {
            let ev = self.read_message();
            if let BrowserEvent::KeyDown(name) = ev {
                println!("received pressed key: {name}");
                let key_name = if let Key::Unicode(char) = key {
                    format!("{char}")
                } else {
                    format!("{key:?}").to_lowercase()
                };
                println!("key_name: {key_name}");
                assert_eq!(key_name, name.to_lowercase());
            } else {
                panic!("BrowserEvent was not a KeyDown: {ev:?}");
            }
        }
        if direction == Release || direction == Click {
            std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect
            let ev = self.read_message();
            if let BrowserEvent::KeyUp(name) = ev {
                println!("received released key: {name}");
                let key_name = if let Key::Unicode(char) = key {
                    format!("{char}")
                } else {
                    format!("{key:?}").to_lowercase()
                };
                println!("key_name: {key_name}");
                assert_eq!(key_name, name.to_lowercase());
            } else {
                panic!("BrowserEvent was not a KeyUp: {ev:?}");
            }
        }
        println!("enigo.key() was a success");
        res
    }

    fn raw(&mut self, keycode: u16, direction: enigo::Direction) -> enigo::InputResult<()> {
        todo!()
    }
}

impl Mouse for EnigoTest {
    fn button(&mut self, button: enigo::Button, direction: Direction) -> enigo::InputResult<()> {
        let res = self.enigo.button(button, direction);
        if direction == Press || direction == Click {
            let ev = self.read_message();
            if let BrowserEvent::MouseDown(name) = ev {
                println!("received pressed button: {name}");
                assert_eq!(button as u32, name);
            } else {
                panic!("BrowserEvent was not a MouseDown: {ev:?}");
            }
        }
        if direction == Release || direction == Click {
            std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect
            let ev = self.read_message();
            if let BrowserEvent::MouseUp(name) = ev {
                println!("received released button: {name}");
                assert_eq!(button as u32, name);
            } else {
                panic!("BrowserEvent was not a MouseUp: {ev:?}");
            }
        }
        println!("enigo.button() was a success");
        res
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> enigo::InputResult<()> {
        // We only change the expected variable on Windows when the ballistic mouse is
        // checked, so it is only needed on Windows
        #[cfg(all(target_os = "windows", feature = "test_mouse"))]
        let (mut x, mut y) = (x, y);

        let res = self.enigo.move_mouse(x, y, coordinate);
        println!("Executed enigo.move_mouse");
        // On Windows the OS either ignores relative mouse moves by 0 or Firefox doesn't
        // create events for them We need to return here so not wait forever for
        // the browser to send a message via websocket
        #[cfg(target_os = "windows")]
        {
            if coordinate == Coordinate::Rel && x == 0 && y == 0 {
                println!("Relative mouse move by (0, 0) detected. We are not waiting for a response, because the OS ignores these calls");
                return res;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect

        let ev = self.read_message();
        println!("Done waiting");

        let mouse_position = if let BrowserEvent::MouseMove(pos_rel, pos_abs) = ev {
            match coordinate {
                Coordinate::Rel => pos_rel,
                Coordinate::Abs => pos_abs,
            }
        } else {
            panic!("BrowserEvent was not a MouseMove: {ev:?}");
        };

        #[cfg(target_os = "windows")]
        {
            #[cfg(not(feature = "test_mouse"))]
            {
                if coordinate == Coordinate::Rel {
                    panic!("If you are testing a relative mouse move on Windows and the mouse is subject to the mouse smoothing curve, the \"test_mouse\" feature must be active");
                }
            }
            #[cfg(feature = "test_mouse")]
            {
                if let Some(test_mouse) = &mut self.test_mouse {
                    let (x_delta, y_delta) = test_mouse
                        .predict_pixel_delta(x, y, coordinate)
                        .expect("Unable to calculate the next position");
                    x = x_delta;
                    y = y_delta;
                };
            }
        }

        assert_eq!(x, mouse_position.0);
        assert_eq!(y, mouse_position.1);

        println!("enigo.move_mouse() was a success");
        res
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> enigo::InputResult<()> {
        let mut length = length;
        let res = self.enigo.scroll(length, axis);
        println!("Executed Enigo");
        std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect

        // On some platforms it is not possible to scroll multiple lines so we
        // repeatedly scroll. In order for this test to work on all platforms, both
        // cases are not differentiated
        let mut mouse_scroll;
        let mut step;
        while length > 0 {
            let ev = self.read_message();
            println!("Done waiting");

            (mouse_scroll, step) =
                if let BrowserEvent::MouseScroll(horizontal_scroll, vertical_scroll) = ev {
                    match axis {
                        Axis::Horizontal => (horizontal_scroll, SCROLL_STEP.0),
                        Axis::Vertical => (vertical_scroll, SCROLL_STEP.1),
                    }
                } else {
                    panic!("BrowserEvent was not a MouseScroll: {ev:?}");
                };
            length -= mouse_scroll / step;
        }

        println!("enigo.scroll() was a success");
        res
    }

    fn main_display(&self) -> enigo::InputResult<(i32, i32)> {
        let res = self.enigo.main_display();
        match res {
            Ok((x, y)) => {
                let (rdev_x, rdev_y) = rdev_main_display();
                println!("enigo display: {x},{y}");
                println!("rdev_display: {rdev_x},{rdev_y}");
                assert_eq!(x, rdev_x);
                assert_eq!(y, rdev_y);
            }
            Err(_) => todo!(),
        }
        res
    }

    // Edge cases don't work (mouse is at the left most border and can't move one to
    // the left)
    fn location(&self) -> enigo::InputResult<(i32, i32)> {
        let res = self.enigo.location();
        match res {
            Ok((x, y)) => {
                let (mouse_x, mouse_y) = mouse_position();
                println!("enigo_position: {x},{y}");
                println!("mouse_position: {mouse_x},{mouse_y}");
                assert_eq!(x, mouse_x);
                assert_eq!(y, mouse_y);
            }
            Err(_) => todo!(),
        }
        res
    }
}

fn rdev_main_display() -> (i32, i32) {
    use rdev::display_size;
    let (x, y) = display_size().unwrap();
    (x.try_into().unwrap(), y.try_into().unwrap())
}

fn mouse_position() -> (i32, i32) {
    use mouse_position::mouse_position::Mouse;

    if let Mouse::Position { x, y } = Mouse::get_mouse_position() {
        (x, y)
    } else {
        panic!("the crate mouse_location was unable to get the position of the mouse");
    }
}
