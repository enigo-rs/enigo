use fixed::{types::extra::U16, FixedI32};

use crate::{
    calc_ballistic_location, get_acceleration,
    tests::mouse,
    Axis::{Horizontal, Vertical},
    Button,
    Coordinate::{Abs, Rel},
    Direction::{Click, Press, Release},
    Enigo, Mouse, Settings,
};
use std::thread;

use super::is_ci;

// TODO: Mouse acceleration on Windows will result in the wrong coordinates when
// doing a relative mouse move. The Github runner has the following settings:
//   MouseSpeed         1
//   MouseThreshold1    6
//   MouseThreshold2   10
//   SmoothMouseCurveX [0, 0.43001, 1.25, 3.86, 40]
//   SmoothMouseCurveY [0, 1.07027, 4.14062, 18.98438, 443.75]
// They can be used to calculate the resulting location even with enabled
// mouse acceleration
fn test_mouse_move(
    enigo: &mut Enigo,
    test_cases: Vec<Vec<((i32, i32), (i32, i32))>>,
    coordinate: crate::Coordinate,
    start: (i32, i32),
) {
    let delay = super::get_delay();
    thread::sleep(delay);

    let error_text = match coordinate {
        Abs => "Failed to move to",
        Rel => "Failed to relatively move to",
    };

    enigo.move_mouse(start.0, start.1, Abs).unwrap(); // Move to absolute start position

    for test_case in test_cases {
        for mouse_action in test_case {
            enigo
                .move_mouse(mouse_action.0 .0, mouse_action.0 .1, coordinate)
                .unwrap();
            thread::sleep(delay);
            let (x_res, y_res) = enigo.location().unwrap();
            assert_eq!(
                (mouse_action.1 .0, mouse_action.1 .1),
                (x_res, y_res),
                "{} {}, {}",
                error_text,
                mouse_action.0 .0,
                mouse_action.0 .1
            );
            thread::sleep(delay);
        }
    }
}

#[test]
// Test the move_mouse function and check it with the mouse_location function
fn unit_move_mouse_to() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // Make a square of 100 pixels starting at the top left corner of the screen and
    // moving down, right, up and left
    let square = vec![
        ((0, 0), (0, 0)),
        ((0, 100), (0, 100)),
        ((100, 100), (100, 100)),
        ((100, 0), (100, 0)),
        ((0, 0), (0, 0)),
    ];
    let test_cases = vec![square];

    test_mouse_move(&mut enigo, test_cases, Abs, (0, 0));
}

#[ignore]
#[test]
// Test the move_mouse function and check it with the mouse_location
// function
fn unit_move_mouse_rel() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    // Make a square of 100 pixels starting at the top left corner of the screen and
    // moving down, right, up and left
    let square = vec![
        ((0, 0), (0, 0)),
        ((0, 100), (0, 100)),
        ((100, 0), (100, 100)),
        ((0, -100), (100, 0)),
        ((-100, 0), (0, 0)),
    ];
    let test_cases = vec![square];

    test_mouse_move(&mut enigo, test_cases, Rel, (0, 0));
}

#[ignore]
#[test]
// Test the move_mouse function and check it with the mouse_location function
fn unit_move_mouse_to_boundaries() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let display_size = enigo.main_display().unwrap();
    println!("Display size {} x {}", display_size.0, display_size.1);

    // Move the mouse outside of the boundaries of the screen
    let screen_boundaries = vec![
        ((-3, 8), (0, 8)),                             // Negative x coordinate
        ((8, -3), (8, 0)),                             // Negative y coordinate
        ((-30, -3), (0, 0)),                           // Try to go to negative x and y coordinates
        ((567_546_546, 20), (display_size.0 - 1, 20)), // Huge x coordinate > screen width
        ((20, 567_546_546), (20, display_size.1 - 1)), // Huge y coordinate > screen heigth
        (
            (567_546_546, 567_546_546),
            (display_size.0 - 1, display_size.1 - 1),
        ), /* Huge x and y coordinate > screen width
                                                        * and screen
                                                        * height */
        ((i32::MAX, 37), (0, 37)),              // Max x coordinate
        ((20, i32::MAX), (20, 0)),              // Max y coordinate
        ((i32::MAX, i32::MAX), (0, 0)),         // Max x and max y coordinate
        ((i32::MAX - 1, i32::MAX - 1), (0, 0)), // Max x and max y coordinate -1
        ((i32::MIN, 20), (0, 20)),              // Min x coordinate
        ((20, i32::MIN), (20, 0)),              // Min y coordinate
        ((i32::MIN, i32::MIN), (0, 0)),         // Min x and min y coordinate
        ((i32::MIN, i32::MAX), (0, 0)),         // Min x and max y coordinate
        ((i32::MAX, i32::MIN), (0, 0)),         // Max x and min y coordinate
    ];
    let test_cases = vec![screen_boundaries];

    test_mouse_move(&mut enigo, test_cases, Abs, (0, 0));
}

#[ignore] // TODO: Mouse acceleration on Windows will result in the wrong coordinates
#[test]
// Test the move_mouse function and check it with the mouse_location
// function
fn unit_move_mouse_rel_boundaries() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let display_size = enigo.main_display().unwrap();
    println!("Display size {} x {}", display_size.0, display_size.1);

    // Move the mouse outside of the boundaries of the screen
    let screen_boundaries = vec![
        ((-3, 8), (0, 8)),                             // Negative x coordinate
        ((8, -10), (8, 0)),                            // Negative y coordinate
        ((-30, -3), (0, 0)),                           // Try to go to negative x and y coordinates
        ((567_546_546, 20), (display_size.0 - 1, 20)), // Huge x coordinate > screen width
        ((20, 567_546_546), (display_size.0 - 1, display_size.1 - 1)), /* Huge y coordinate >
                                                        * screen heigth */
        (
            (567_546_546, 567_546_546),
            (display_size.0 - 1, display_size.1 - 1),
        ), /* Huge x and y coordinate > screen width
            * and screen
            * height */
        ((-display_size.0, -display_size.1), (0, 0)), // Reset to (0,0)
        ((i32::MAX, 37), (0, 37)),                    // Max x coordinate
        ((20, i32::MAX), (20, 37)),                   // Max y coordinate
        ((i32::MAX, i32::MAX), (0, 0)),               // Max x and max y coordinate
        ((i32::MAX - 1, i32::MAX - 1), (0, 0)),       // Max x and max y coordinate -1
        ((i32::MIN, 20), (0, 20)),                    // Min x coordinate
        ((20, i32::MIN), (20, 0)),                    // Min y coordinate
        ((i32::MIN, i32::MIN), (0, 0)),               // Min x and min y coordinate
        ((i32::MIN, i32::MAX), (0, 0)),               // Min x and max y coordinate
        ((i32::MAX, i32::MIN), (0, 0)),               // Max x and min y coordinate
    ];
    let test_cases = vec![screen_boundaries];

    test_mouse_move(&mut enigo, test_cases, Rel, (0, 0));
}

#[test]
// Test the main_display function
// The CI's virtual display has a dimension of 1024x768 (except on macOS where
// it is 1920x1080). If the test is ran outside of the CI, we don't know the
// displays dimensions so we just make sure it is greater than 0x0.
fn unit_display_size() {
    let enigo = Enigo::new(&Settings::default()).unwrap();
    let display_size = enigo.main_display().unwrap();
    println!("Main display size: {}x{}", display_size.0, display_size.1);
    if !is_ci() {
        assert!(display_size.0 > 0);
        assert!(display_size.1 > 0);
        return;
    }

    let ci_display = if cfg!(target_os = "macos") {
        (1920, 1080)
    } else {
        (1024, 768)
    };

    assert_eq!(
        (display_size.0, display_size.1),
        (ci_display.0, ci_display.1)
    );
}

#[test]
// Test all the mouse buttons, make sure none of them panic
fn unit_button_click() {
    use strum::IntoEnumIterator;

    thread::sleep(super::get_delay());
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    for button in Button::iter() {
        assert_eq!(
            enigo.button(button, Press),
            Ok(()),
            "Didn't expect an error for button: {button:#?}"
        );
        assert_eq!(
            enigo.button(button, Release),
            Ok(()),
            "Didn't expect an error for button: {button:#?}"
        );
        assert_eq!(
            enigo.button(button, Click),
            Ok(()),
            "Didn't expect an error for button: {button:#?}"
        );
    }
}

#[test]
// Click each mouse button ten times, make sure none of them panic
fn unit_10th_click() {
    use strum::IntoEnumIterator;

    thread::sleep(super::get_delay());
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    for button in Button::iter() {
        for _ in 0..10 {
            assert_eq!(
                enigo.button(button, Click),
                Ok(()),
                "Didn't expect an error for button: {button:#?}"
            );
        }
    }
}

#[ignore] // Hangs with x11rb
#[test]
fn unit_scroll() {
    let delay = super::get_delay();
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let test_cases = vec![0, 1, 5, 100, 57899, -57899, -0, -1, -5, -100];

    for length in &test_cases {
        thread::sleep(delay);
        assert_eq!(
            enigo.scroll(*length, Horizontal),
            Ok(()),
            "Didn't expect an error when horizontally scrolling: {length}"
        );
    }
    for length in &test_cases {
        thread::sleep(delay);
        assert_eq!(
            enigo.scroll(*length, Vertical),
            Ok(()),
            "Didn't expect an error when vertically scrolling: {length}"
        );
    }
}

#[ignore] // Contains a relative mouse move so it does not work on Windows
#[test]
// Press down and drag the mouse
fn unit_mouse_drag() {
    let delay = super::get_delay();
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.move_mouse(500, 200, Abs).unwrap();
    enigo.button(Button::Left, Press).unwrap();
    enigo.move_mouse(100, 100, Rel).unwrap();
    thread::sleep(delay);
    enigo.button(Button::Left, Release).unwrap();
}

#[test]
fn unit_rel_mouse_move() {
    #[cfg(target_os = "windows")]
    use crate::{mouse_thresholds_and_acceleration, set_mouse_thresholds_and_acceleration};
    use crate::{
        Coordinate::{Abs, Rel},
        Enigo, Mouse as _, Settings,
    };

    let delay = super::get_delay();

    // the tests don't work if the mouse is subject to mouse speed and acceleration
    // level
    #[cfg(target_os = "windows")]
    let (threshold1, threshold2, acceleration_level) = {
        let (threshold1, threshold2, acceleration_level) =
            mouse_thresholds_and_acceleration().expect("Unable to get the mouse threshold");

        if acceleration_level != 0 {
            set_mouse_thresholds_and_acceleration(threshold1, threshold2, 0)
                .expect("Unable to set the mouse threshold");
        }
        (threshold1, threshold2, acceleration_level)
    };

    let mut enigo = Enigo::new(&Settings {
        windows_subject_to_mouse_speed_and_acceleration_level: true,
        ..Default::default()
    })
    .unwrap();

    let test_cases = vec![
        ((100, 100), Abs),
        ((0, 0), Rel),
        ((-0, 0), Rel),
        ((0, -0), Rel),
        ((-0, -0), Rel),
        ((1, 0), Rel),
        ((0, 1), Rel),
        ((-1, -1), Rel),
        ((4, 6), Rel),
        ((-42, 63), Rel),
        ((12, -20), Rel),
        ((-43, -1), Rel),
        ((200, 200), Rel),
        ((-200, 200), Rel),
        ((200, -200), Rel),
        ((-200, -200), Rel),
    ];

    let mut expected_location = (0, 0);
    for ((x, y), coord) in test_cases {
        match coord {
            Abs => {
                expected_location = (x, y);
            }
            Rel => {
                expected_location.0 += x;
                expected_location.1 += y;
            }
        };

        enigo.move_mouse(x, y, coord).unwrap();
        thread::sleep(delay);
        let actual_location = enigo.location().unwrap();
        assert_eq!(
            expected_location, actual_location,
            "test case: ({x},{y}) {coord:?}\n expected_location: {expected_location:?}\n actual_location: {actual_location:?}"
        );
    }

    #[cfg(target_os = "windows")]
    // restore the previous setting
    set_mouse_thresholds_and_acceleration(threshold1, threshold2, acceleration_level)
        .expect("Unable to restore the old mouse threshold");
}

#[test]
// Test the calculation of the ballistic mouse
fn unit_ballistic_calc() {
    use fixed::FixedI32;
    let mouse_curves = vec![[
        [
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
            FixedI32::from_le_bytes([0x00, 0x00, 0x64, 0x00]), // 0.43
            FixedI32::from_le_bytes([0x00, 0x00, 0x96, 0x00]), // 1.25
            FixedI32::from_le_bytes([0x00, 0x00, 0xC8, 0x00]), // 3.86
            FixedI32::from_le_bytes([0x00, 0x00, 0xFA, 0x00]), // 40.0
        ],
        [
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
            FixedI32::from_le_bytes([0xCD, 0x4C, 0x18, 0x00]), // 0.43
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 1.25
            FixedI32::from_le_bytes([0xCD, 0x4C, 0x18, 0x00]), // 3.86
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 40.0
        ],
    ]];
    let test_case = [
        (1, 0),
        (120, 6),
        (350, 19),
        (430, 10),
        (530, 0),
        (640, 12),
        (700, 19),
        (835, 4),
    ];

    let remainder_x = FixedI32::from_num(0);
    let remainder_y = FixedI32::from_num(0);

    let mouse_speed = crate::mouse_speed().unwrap();
    let mouse_speed = crate::update_mouse_speed(mouse_speed).unwrap();
    let mouse_speed = FixedI32::<U16>::checked_from_num(mouse_speed).unwrap();

    for curve in mouse_curves {
        for (x, correct_x) in test_case {
            println!("\n{x}");
            let ((new_x, _), _) =
                calc_ballistic_location(x, 0, remainder_x, remainder_y, mouse_speed, curve)
                    .unwrap();
            assert!(i32::abs(correct_x - new_x.to_num::<i32>()) <= 1, "i: {x}");
        }
    }
}

#[test]
fn unit_acceleration() {
    const DEFAULT_SCREEN_UPDATE_RATE: i32 = 75; // in HZ
    const DEFAULT_SCREEN_RESOLUTION: i32 = 96; // in DPI

    let mouse_curves = [
        [
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
            FixedI32::from_le_bytes([0x00, 0x00, 0x64, 0x00]), // 0.43
            FixedI32::from_le_bytes([0x00, 0x00, 0x96, 0x00]), // 1.25
            FixedI32::from_le_bytes([0x00, 0x00, 0xC8, 0x00]), // 3.86
            FixedI32::from_le_bytes([0x00, 0x00, 0xFA, 0x00]), // 40.0
        ],
        [
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
            FixedI32::from_le_bytes([0xCD, 0x4C, 0x18, 0x00]), // 0.43
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 1.25
            FixedI32::from_le_bytes([0xCD, 0x4C, 0x18, 0x00]), // 3.86
            FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 40.0
        ],
    ];

    let screen_update_rate = FixedI32::<U16>::from_num(DEFAULT_SCREEN_UPDATE_RATE);
    //let screen_resolution = system_dpi();
    //println!("DPI: {screen_resolution}");
    // let screen_resolution = FixedI32::<U16>::from_num(screen_resolution);
    let screen_resolution = FixedI32::<U16>::from_num(DEFAULT_SCREEN_RESOLUTION);
    let v_pointer_factor = screen_update_rate.checked_div(screen_resolution).unwrap();

    let scaled_smooth_mouse_curve_x: Vec<_> = mouse_curves[0]
        .iter()
        .map(|&v| v.checked_mul(FixedI32::<U16>::from_num(3.5)).unwrap())
        .collect();
    let scaled_smooth_mouse_curve_y: Vec<_> = mouse_curves[1]
        .iter()
        .map(|&v| v.checked_div(v_pointer_factor).unwrap())
        .collect();

    let mouse_curves = [
        scaled_smooth_mouse_curve_x.try_into().unwrap(),
        scaled_smooth_mouse_curve_y.try_into().unwrap(),
    ];

    let test_case = [
        (1, 0),
        (120, 6),
        (350, 19),
        (430, 10),
        (530, 0),
        (640, 12),
        (700, 19),
        (835, 4),
    ];
    for test in test_case {
        let magnitude = FixedI32::from_num(test.0);
        let acceleration = get_acceleration(magnitude, mouse_curves).unwrap();
        assert_eq!(acceleration.to_num::<i32>(), test.1, "x: {}", test.0);
    }
}
