use std::sync::mpsc::Receiver;

use enigo::{Enigo, MouseControllable};

use super::BrowserEvent;

const ERROR: i32 = 2;

pub fn run(recv: &Receiver<BrowserEvent>) {
    println!("Move mouse");
    set(recv, (100, 100));
    println!("Move mouse");
    set(recv, (200, 200));
    println!("Rel move mouse");
    rel(recv, (20, 20));
    println!("Rel move mouse");
    rel(recv, (-20, 20));
    println!("Rel move mouse");
    rel(recv, (20, -20));
    println!("Rel move mouse");
    rel(recv, (-20, -20));
    println!("Scroll");
    scroll(recv);
}

fn set(recv: &Receiver<BrowserEvent>, position: (i32, i32)) {
    Enigo::new().mouse_move_to(position.0, position.1);
    println!("Executed Enigo");
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    println!("Done waiting");
    if let BrowserEvent::MouseMove(pos) = ev {
        assert!((position.0 - pos.1 .0).abs() <= ERROR);
        assert!((position.1 - pos.1 .1).abs() <= ERROR);
        println!("Move success");
    } else {
        panic!("Event wasn't MouseMove after mouse::set. {ev:?}");
    }
}

fn rel(recv: &Receiver<BrowserEvent>, offset: (i32, i32)) {
    Enigo::new().mouse_move_relative(offset.0, offset.1);
    println!("Executed Enigo");
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    println!("Done waiting");
    if let BrowserEvent::MouseMove(pos) = ev {
        assert!((offset.0 - pos.0 .0).abs() <= ERROR);
        assert!((offset.1 - pos.0 .1).abs() <= ERROR);
        println!("Rel move success");
    } else {
        panic!("Event wasn't MouseMove after mouse::rel. {ev:?}");
    }
}

fn scroll(recv: &Receiver<BrowserEvent>) {
    Enigo::new().mouse_scroll_x(1);
    println!("Executed Enigo");
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    println!("Done waiting");
    if let BrowserEvent::MouseWheel(length) = ev {
        println!("Scroll success");
        assert!(length.0 > 0);
        assert!(length.1 == 0);
    } else {
        panic!("Event wasn't MouseWheel after mouse::scroll. {ev:?}");
    }
    Enigo::new().mouse_scroll_y(1);
    println!("Executed Enigo");
    let ev = recv
        .recv_timeout(std::time::Duration::from_millis(5000))
        .unwrap();
    println!("Done waiting");
    if let BrowserEvent::MouseWheel(length) = ev {
        println!("Scroll success");
        assert!(length.0 == 0);
        assert!(length.1 > 0);
    } else {
        panic!("Event wasn't MouseWheel after mouse::scroll. {ev:?}");
    }
}
