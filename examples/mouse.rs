use enigo::{
    Button,
    Direction::{Click, Press, Release},
    Enigo, Mouse, Settings,
    {Axis::Horizontal, Axis::Vertical},
    {Coordinate::Abs, Coordinate::Rel},
};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::try_init().ok();
    let wait_time = Duration::from_secs(2);
    let mut enigo = Enigo::new(&Settings {
        windows_subject_to_mouse_speed_and_acceleration_level: true,
        ..Default::default()
    })
    .unwrap();

    thread::sleep(Duration::from_secs(4));
    println!("screen dimensions: {:?}", enigo.main_display().unwrap());
    println!("mouse location: {:?}", enigo.location().unwrap());

    enigo.move_mouse(0, 0, Abs).unwrap();
    let detail = 1;
    for i in 0..600 {
        for _ in 0..detail {
            enigo.move_mouse(i, 0, Rel).unwrap();
        }

        //thread::sleep(wait_time);
        println!("{i}, {:?},", enigo.location().unwrap().0);
        for _ in 0..detail {
            enigo.move_mouse(-i, 0, Rel).unwrap();
        }
    }
    println!("\n\ny");

    for i in 0..600 {
        for _ in 0..detail {
            enigo.move_mouse(0, i, Rel).unwrap();
        }

        //thread::sleep(wait_time);
        println!("{i}, {:?},", enigo.location().unwrap().0);
        for _ in 0..detail {
            enigo.move_mouse(0, -i, Rel).unwrap();
        }
    }
}
