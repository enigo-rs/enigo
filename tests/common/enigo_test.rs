use std::net::{TcpListener, TcpStream};

use tungstenite::accept;

use enigo::{
    Axis, Button,
    Coordinate::{self, Abs},
    Direction::{self, Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};

use super::browser_events::BrowserEvent;

const TIMEOUT: u64 = 5; // Number of minutes the test is allowed to run before timing out
                        // This is needed, because some of the websocket functions are blocking and
                        // would run indefinitely without a timeout if they don't receive a message
const INPUT_DELAY: u64 = 40; // Number of milliseconds to wait for the input to have an effect
const SCROLL_STEP: (i32, i32) = (20, 114); // (horizontal, vertical)

pub struct EnigoTest {
    enigo: Enigo,
    websocket: tungstenite::WebSocket<TcpStream>,
}

impl EnigoTest {
    pub fn new(settings: &Settings) -> Self {
        env_logger::init();
        EnigoTest::start_timeout_thread();
        let enigo = Enigo::new(settings).unwrap();
        let _ = &*super::browser::BROWSER_INSTANCE; // Launch Firefox
        let websocket = Self::websocket();

        std::thread::sleep(std::time::Duration::from_secs(10)); // Give Firefox some time to launch
        Self { enigo, websocket }
    }

    // Maximize Firefox by pressing keys or moving the mouse
    pub fn maximize_browser(&mut self) {
        if cfg!(target_os = "macos") {
            //  self.key(Key::Control, Press).unwrap();
            //  self.key(Key::Meta, Press).unwrap();
            self.key(Key::Unicode('f'), Click).unwrap();
            //  self.key(Key::Meta, Release).unwrap();
            //  self.key(Key::Control, Release).unwrap();
        } else {
            self.key(Key::F11, Click).unwrap();
            self.move_mouse(200, 200, Abs).unwrap();
            self.button(Button::Left, Click).unwrap();
        };

        // Wait for full screen animation
        std::thread::sleep(std::time::Duration::from_secs(10));
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
        if let BrowserEvent::ReadyForText = self.read_message() {
            println!("Ready for the text");
        } else {
            panic!("Failed to get ready for the text");
        };
        let res = self.enigo.text(text);
        std::thread::sleep(std::time::Duration::from_millis(INPUT_DELAY)); // Wait for input to have an effect
        self.send_message("GetText");

        loop {
            if let BrowserEvent::Text(received_text) = self.read_message() {
                println!("received text: {received_text}");
                assert_eq!(text, received_text);
                break;
            }
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
        let res = self.enigo.move_mouse(x, y, coordinate);
        println!("Executed enigo.move_mouse");
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
