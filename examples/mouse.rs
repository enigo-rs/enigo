use enigo::{
    set_mouse_thresholds_and_acceleration,
    Coordinate::{Abs, Rel},
    Enigo, Mouse, Settings,
};
use fixed::{types::extra::U16, FixedI32};
use std::thread;
use std::time::Duration;

fn main() {
    env_logger::try_init().ok();

    thread::sleep(Duration::from_secs(2));

    set_mouse_thresholds_and_acceleration(6, 10, 1).unwrap();

    let [curve_x, curve_y] = enigo::mouse_curve(true, true).unwrap();
    println!("mouse curve x: {curve_x:?}");
    println!("mouse curve y: {curve_y:?}");

    let mut enigo = Enigo::new(&Settings {
        windows_subject_to_mouse_speed_and_acceleration_level: true,
        ..Default::default()
    })
    .unwrap();

    let (start_x, start_y) = (0, 0);
    enigo.move_mouse(start_x, start_y, Abs).unwrap();

    let mut actually = enigo.location().unwrap();
    println!("mouse location: {actually:?}");
    println!();

    let (mut remainder_x, mut remainder_y) =
        (FixedI32::<U16>::from_num(0), FixedI32::<U16>::from_num(0));

    println!("Factors: ");
    for i in 0..1000 {
        // Do it ten times to be more precise (remainders)
        for _ in 0..10 {
            enigo.move_mouse(i, 0, Rel).unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(30));
        actually = enigo.location().unwrap();

        for _ in 0..10 {
            enigo.move_mouse(-i, 0, Rel).unwrap();
        }
        println!("{}", actually.0);
        println!("{i}; {};", (actually.0 as f64 / 10.0) / i as f64);

        /*
        println!("rel move by: ({i}, 0)");
        enigo.move_mouse(i, 0, Rel).unwrap();
        let ((mut ballistic_x, mut ballistic_y), (r_x, r_y)) = enigo::calc_ballistic_location(
            i,
            0,
            remainder_x,
            remainder_y,
            [curve_x.unwrap(), curve_y.unwrap()],
        )
        .unwrap();

        ballistic_x += FixedI32::<U16>::from_num(actually.0);
        ballistic_y += FixedI32::<U16>::from_num(actually.1);
        remainder_x = r_x;
        remainder_y = r_y;

        actually = enigo.location().unwrap();
        println!(
            " ballistic: ({ballistic_x:?}, {ballistic_y:?})\n actually: ({:?}, {:?})\n ballistic/actually: {}",
            actually.0,
            actually.1,
            ballistic_x.to_num::<f64>() / actually.0 as f64
        );*/
    }
}
