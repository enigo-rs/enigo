use enigo::Direction;
use serde::{Deserialize, Serialize};
use tungstenite::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowserEventError {
    UnknownMessageType,
    ParseError,
    WebsocketClosed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrowserEvent {
    pub text: String,
    pub event: Event,
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")] // use the "type" field to decide which variant to pick
pub enum Event {
    #[serde(rename_all = "camelCase")]
    Key {
        timestamp: f64,
        key: String,
        code: String,
        key_code: u16,
        alt_key: bool,
        ctrl_key: bool,
        shift_key: bool,
        meta_key: bool,
        direction: Direction,
    },
    #[serde(rename_all = "camelCase")]
    Button {
        timestamp: f64,
        button: u8,
        buttons: u8,
        client_x: i32,
        client_y: i32,
        screen_x: i32,
        screen_y: i32,
        direction: Direction,
    },
    #[serde(rename_all = "camelCase")]
    MouseMove {
        timestamp: f64,
        client_x: i32,
        client_y: i32,
        movement_x: i32,
        movement_y: i32,
    },
    #[serde(rename_all = "camelCase")]
    Scroll {
        timestamp: f64,
        delta_x: f64,
        delta_y: f64,
        delta_mode: i32,
        client_x: i32,
        client_y: i32,
    },
}

impl TryFrom<Message> for BrowserEvent {
    type Error = BrowserEventError;

    fn try_from(message: Message) -> Result<Self, Self::Error> {
        match message {
            Message::Close(_) => {
                println!("Message::Close received");
                Err(BrowserEventError::WebsocketClosed)
            }
            Message::Text(msg) => {
                println!("Browser received input");
                println!("msg: {msg:?}");

                // Attempt to deserialize the text message into a BrowserEvent
                if let Ok(event) = serde_json::from_str::<BrowserEvent>(&msg) {
                    Ok(event)
                } else {
                    println!("Parse error! Message: {msg}");
                    Err(BrowserEventError::ParseError)
                }
            }
            Message::Binary(_) | Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                println!("Other Message received");
                Err(BrowserEventError::UnknownMessageType)
            }
        }
    }
}

#[test]
fn deserialize_browser_events() {
    use enigo::Direction;
    use tungstenite::Message;

    let cases: Vec<(&str, BrowserEvent)> = vec![
        (
            r#"{"text":"hello","event":{"type":"key","timestamp":1000.0,"key":"a","code":"KeyA","keyCode":65,"altKey":false,"ctrlKey":false,"shiftKey":true,"metaKey":false,"direction":"pressed"}}"#,
            BrowserEvent {
                text: "hello".into(),
                event: Event::Key {
                    timestamp: 1000.0,
                    key: "a".into(),
                    code: "KeyA".into(),
                    key_code: 65,
                    alt_key: false,
                    ctrl_key: false,
                    shift_key: true,
                    meta_key: false,
                    direction: Direction::Press,
                },
            },
        ),
        (
            r#"{"text":"dfdsfsdd","event":{"type":"key","timestamp":1391401.1000000006,"key":"d","code":"KeyD","keyCode":68,"altKey":false,"ctrlKey":false,"shiftKey":false,"metaKey":false,"direction":"released"}}"#,
            BrowserEvent {
                text: "dfdsfsdd".into(),
                event: Event::Key {
                    timestamp: 1391401.1000000006,
                    key: "d".into(),
                    code: "KeyD".into(),
                    key_code: 68,
                    alt_key: false,
                    ctrl_key: false,
                    shift_key: false,
                    meta_key: false,
                    direction: Direction::Release,
                },
            },
        ),
        (
            r#"{"text":"","event":{"type":"button","timestamp":2000.0,"button":0,"buttons":1,"clientX":300,"clientY":400,"screenX":500,"screenY":600,"direction":"pressed"}}"#,
            BrowserEvent {
                text: "".into(),
                event: Event::Button {
                    timestamp: 2000.0,
                    button: 0,
                    buttons: 1,
                    client_x: 300,
                    client_y: 400,
                    screen_x: 500,
                    screen_y: 600,
                    direction: Direction::Press,
                },
            },
        ),
        (
            r#"{"text":"","event":{"type":"button","timestamp":2001.0,"button":0,"buttons":0,"clientX":300,"clientY":400,"screenX":500,"screenY":600,"direction":"released"}}"#,
            BrowserEvent {
                text: "".into(),
                event: Event::Button {
                    timestamp: 2001.0,
                    button: 0,
                    buttons: 0,
                    client_x: 300,
                    client_y: 400,
                    screen_x: 500,
                    screen_y: 600,
                    direction: Direction::Release,
                },
            },
        ),
        (
            r#"{"text":"","event":{"type":"mouseMove","timestamp":1273293.2999999998,"clientX":101,"clientY":260,"movementX":14,"movementY":52}}"#,
            BrowserEvent {
                text: "".into(),
                event: Event::MouseMove {
                    timestamp: 1273293.2999999998,
                    client_x: 101,
                    client_y: 260,
                    movement_x: 14,
                    movement_y: 52,
                },
            },
        ),
        (
            r#"{"text":"","event":{"type":"scroll","timestamp":54321.0,"deltaX":0.0,"deltaY":3.0,"deltaMode":0,"clientX":150,"clientY":250}}"#,
            BrowserEvent {
                text: "".into(),
                event: Event::Scroll {
                    timestamp: 54321.0,
                    delta_x: 0.0,
                    delta_y: 3.0,
                    delta_mode: 0,
                    client_x: 150,
                    client_y: 250,
                },
            },
        ),
    ];

    for (raw, expected) in cases {
        // serialize back to JSON for comparison
        let expected_json = serde_json::to_string(&expected).unwrap();
        println!("expected = {}", expected_json);
        println!("raw      = {}\n", raw);

        // parse the JSON
        let msg = Message::Text(raw.into());
        let parsed = BrowserEvent::try_from(msg).unwrap();

        assert_eq!(parsed, expected);
    }
}
