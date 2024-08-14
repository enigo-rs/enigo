use serde::{Deserialize, Serialize};
use tungstenite::Message;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowserEvent {
    Text(String),
    KeyDown(String),
    KeyUp(String),
    MouseDown(u32),
    MouseUp(u32),
    MouseMove((i32, i32), (i32, i32)), // (relative, absolute)
    MouseScroll(i32, i32),
    Open,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrowserEventError {
    UnknownMessageType,
    ParseError,
}

impl TryFrom<Message> for BrowserEvent {
    type Error = BrowserEventError;

    fn try_from(message: Message) -> Result<Self, Self::Error> {
        match message {
            Message::Close(_) => {
                println!("Message::Close received");
                Ok(BrowserEvent::Close)
            }
            Message::Text(msg) => {
                println!("Message::Text received");
                println!("msg: {msg:?}");

                // Attempt to deserialize the text message into a BrowserEvent
                if let Ok(event) = ron::from_str::<BrowserEvent>(&msg) {
                    Ok(event)
                } else {
                    println!("Parse error");
                    Err(BrowserEventError::ParseError)
                }
            }
            _ => {
                println!("Other Message received");
                Err(BrowserEventError::UnknownMessageType)
            }
        }
    }
}

#[test]
fn deserialize_browser_events() {
    let messages = vec![
        (
            Message::Text("Text(\"Testing\")".to_string()),
            BrowserEvent::Text("Testing".to_string()),
        ),
        (
            Message::Text("Text(\"Hi how are you?❤️ äüß$3\")".to_string()),
            BrowserEvent::Text("Hi how are you?❤️ äüß$3".to_string()),
        ),
        (
            Message::Text("KeyDown(\"F11\")".to_string()),
            BrowserEvent::KeyDown("F11".to_string()),
        ),
        (
            Message::Text("KeyUp(\"F11\")".to_string()),
            BrowserEvent::KeyUp("F11".to_string()),
        ),
        (
            Message::Text("MouseDown(0)".to_string()),
            BrowserEvent::MouseDown(0),
        ),
        (
            Message::Text("MouseUp(0)".to_string()),
            BrowserEvent::MouseUp(0),
        ),
        (
            Message::Text("MouseMove((-1806, -487), (200, 200))".to_string()),
            BrowserEvent::MouseMove((-1806, -487), (200, 200)),
        ),
        (
            Message::Text("MouseScroll(3, -2)".to_string()),
            BrowserEvent::MouseScroll(3, -2),
        ),
    ];

    for (msg, event) in messages {
        let serialized = ron::to_string(&event).unwrap();
        println!("serialized = {serialized}");

        assert!(BrowserEvent::try_from(msg).unwrap() == event);
    }
}
