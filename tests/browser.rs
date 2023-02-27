use enigo::{Key, KeyboardControllable};
use std::sync::mpsc::channel;

mod common;
use common::BrowserEvent;

#[test]
#[ignore]
fn browser_events() {
    let (tx, rs) = channel::<BrowserEvent>();
    println!("Created channel");
    std::thread::spawn(move || common::launch_ws_server(tx));
    println!("WebSocket server thread was spawned");
    std::thread::sleep(std::time::Duration::from_millis(10000)); // Wait a few seconds to make sure the browser was started
    common::launch_browser(&rs);
    println!("Browser was launched");

    enigo::Enigo::new().key_click(Key::F11);
    // Full screen animation
    std::thread::sleep(std::time::Duration::from_millis(3000));
    rs.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap(); // KeyDown("F11")
    rs.recv_timeout(std::time::Duration::from_millis(500))
        .unwrap(); // KeyUp("F11")

    common::mouse::run(&rs);
    println!("Mouse test successfull");
    common::key::run(&rs);
    println!("Keyboard test successfull");
    println!("All tests successfull");
}

/*
#[test]
#[ignore]
fn run_ws_server() {
    let (tx, _rs) = channel::<BrowserEvent>();
    println!("Created channel");
    std::thread::spawn(move || common::launch_ws_server(tx));
    std::thread::sleep(std::time::Duration::from_millis(100000)); // Sleep in order to continue running the WebSocket server in another thread
}
// */
