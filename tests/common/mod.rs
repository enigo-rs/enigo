use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};

use tungstenite::{accept, Message};

pub mod key;
pub mod mouse;

#[derive(Debug, PartialEq)]
pub enum BrowserEvent {
    KeyDown(String),
    KeyUp(String),
    MouseDown(String),
    MouseUp(String),
    MouseMove(((i32, i32), (i32, i32))),
    MouseWheel((i32, i32)),
    Open,
    Close,
}

#[allow(clippy::similar_names)]
fn handle_connection(stream: TcpStream, tx: &Sender<BrowserEvent>) {
    let mut websocket = accept(stream).unwrap();

    println!("Start waiting for messages");
    loop {
        let message = websocket.read_message().unwrap();
        println!("Start processing message");

        match message {
            Message::Close(_) => {
                println!("Message::Close received");
                tx.send(BrowserEvent::Close).unwrap();
                println!("Client disconnected");
                return;
            }
            Message::Text(msg) => {
                println!("Message::Text received");
                println!("msg: {msg:?}");
                let (key, data) = msg.split_once(':').unwrap();
                let be = match key {
                    "open" => BrowserEvent::Open,
                    "close" => BrowserEvent::Close, // Is this needed?
                    "keydown" => BrowserEvent::KeyDown(data.to_string()),
                    "keyup" => BrowserEvent::KeyUp(data.to_string()),
                    "mousedown" => BrowserEvent::MouseDown(data.to_string()),
                    "mouseup" => BrowserEvent::MouseUp(data.to_string()),
                    "mousemove" => {
                        // format is relx,rely|absx,absy
                        let (rel, abs) = data.split_once('|').unwrap();
                        let (relx, rely) = rel.split_once(',').unwrap();
                        let (absx, absy) = abs.split_once(',').unwrap();
                        BrowserEvent::MouseMove((
                            (relx.parse().unwrap(), rely.parse().unwrap()),
                            (absx.parse().unwrap(), absy.parse().unwrap()),
                        ))
                    }
                    "mousewheel" => {
                        // format is x,y
                        let (x, y) = data.split_once(',').unwrap();
                        BrowserEvent::MouseWheel((x.parse().unwrap(), y.parse().unwrap()))
                    }
                    _ => {
                        println!("Other text received");
                        continue;
                    }
                };
                tx.send(be).unwrap();
            }
            _ => {
                println!("Other Message received");
            }
        }
    }
}

pub fn launch_ws_server(tx: Sender<BrowserEvent>) {
    let listener = TcpListener::bind("127.0.0.1:26541").unwrap();
    println!("TcpListener was created");

    match listener.accept() {
        Ok((stream, addr)) => {
            println!("New connection was made from {addr:?}");
            std::thread::spawn(move || handle_connection(stream, &tx));
        }
        Err(e) => {
            println!("Connection failed: {e:?}");
        }
    }
}

pub fn launch_browser(rs: &Receiver<BrowserEvent>) {
    let url = &format!(
        "file://{}/tests/index.html",
        std::env::current_dir().unwrap().to_str().unwrap()
    );
    if !webbrowser::Browser::Firefox.exists() {
        println!("Firefox is not installed");
    }
    if webbrowser::open_browser_with_options(
        webbrowser::Browser::Default,
        url,
        webbrowser::BrowserOptions::new().with_suppress_output(false),
    )
    .is_err()
    {
        panic!("Unable to open the browser");
    }
    println!("Try opening test page");
    if rs.recv_timeout(std::time::Duration::from_millis(5000)) == Ok(BrowserEvent::Open) {
        println!("Test page was opened");
    } else {
        panic!("Expected Open event");
    }
    /*loop {
        if rs
            .recv_timeout(std::time::Duration::from_millis(500))
            .is_err()
        {
            break;
        }
    }*/
    println!("Done with launch function");
}
