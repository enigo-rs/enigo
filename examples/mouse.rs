use enigo::{Button, Enigo, MouseControllable, Settings};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::init();
    let wait_time = Duration::from_secs(2);
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    thread::sleep(Duration::from_secs(4));
    println!("screen dimensions: {:?}", enigo.main_display_size());
    println!("mouse location: {:?}", enigo.mouse_location());

    thread::sleep(wait_time);

    enigo.mouse_move_to(500, 200);
    thread::sleep(wait_time);

    enigo.mouse_down(Button::Left);
    thread::sleep(wait_time);

    enigo.mouse_move_relative(100, 100);
    thread::sleep(wait_time);

    enigo.mouse_up(Button::Left);
    thread::sleep(wait_time);

    enigo.mouse_click(Button::Left);
    thread::sleep(wait_time);

    enigo.mouse_scroll_x(2);
    thread::sleep(wait_time);

    enigo.mouse_scroll_x(-2);
    thread::sleep(wait_time);

    enigo.mouse_scroll_y(2);
    thread::sleep(wait_time);

    enigo.mouse_scroll_y(-2);
    thread::sleep(wait_time);

    println!("mouse location: {:?}", enigo.mouse_location());
}
