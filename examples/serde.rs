use enigo::{
    Button, Enigo, Key, Settings,
    agent::{Agent, Token},
};
use std::{thread, time::Duration};

fn main() {
    env_logger::try_init().ok();
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // write text, move the mouse (10/10) relative from the cursors position, scroll
    // down, enter the unicode U+1F525 (🔥) and then select all
    let tokens = vec![
        Token::Text("Hello World! ❤️".to_string()),
        Token::MoveMouse(10, 10, enigo::Coordinate::Rel),
        Token::Scroll(5, enigo::Axis::Vertical),
        Token::Button(Button::Left, enigo::Direction::Click),
        Token::Key(Key::Unicode('🔥'), enigo::Direction::Click),
        Token::Key(Key::Control, enigo::Direction::Press),
        Token::Key(Key::Unicode('a'), enigo::Direction::Click),
        Token::Key(Key::Control, enigo::Direction::Release),
    ];

    // There are serde aliases so you could also deserialize the same tokens from
    // the following string let serialized=r#"[t("Hello World!
    // ❤\u{fe0f}"),m(10,10,r),s(5),b(l),k(uni('🔥')),k(ctrl,p),k(uni('a')),
    // k(ctrl,r)]"#.to_string();
    let serialized = ron::to_string(&tokens).unwrap();
    println!("serialized = {serialized}");

    let deserialized_tokens: Vec<_> = ron::from_str(&serialized).unwrap();
    for token in &deserialized_tokens {
        enigo.execute(token).unwrap();
    }
}
