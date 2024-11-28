use fixed::{types::extra::U16, FixedI32};
use log::debug;

use crate::{Coordinate, InputError};

// const DEFAULT_BUS_UPDATE_RATE: i32 = 125; // in HZ
// const DEFAULT_POINTER_RESOLUTION: i32 = 400; // in mickey/inch
pub const DEFAULT_SCREEN_RESOLUTION: i32 = 96; // in DPI
pub const DEFAULT_SCREEN_UPDATE_RATE: i32 = 75; // in HZ

/// Struct that will calculate the resulting position of the mouse. This will
/// NOT simulate a mouse move. It's pretty much only useful for testing or if
/// you want the mouse to behave similar to on Windows on other platforms
pub struct TestMouse {
    ballistic: bool,
    x_abs_fix: FixedI32<U16>,
    y_abs_fix: FixedI32<U16>,
    remainder_x: FixedI32<U16>,
    remainder_y: FixedI32<U16>,
    mouse_speed: FixedI32<U16>,
    p_mouse_factor: FixedI32<U16>,
    v_pointer_factor: FixedI32<U16>,
    smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
}

impl Default for TestMouse {
    fn default() -> Self {
        #[cfg(not(target_os = "windows"))]
        let acceleration_level = 0;
        #[cfg(target_os = "windows")]
        let (_, _, acceleration_level) =
            crate::mouse_thresholds_and_acceleration().expect("Unable to get the mouse threshold");
        // We only have to do a ballistic calculation if the acceleration level is 1
        let ballistic = acceleration_level == 1;

        #[cfg(not(target_os = "windows"))]
        let mouse_speed = FixedI32::<U16>::from_num(1.0);
        #[cfg(target_os = "windows")]
        let mouse_speed = {
            let mouse_speed = crate::mouse_speed().unwrap();
            let mouse_speed = TestMouse::mouse_sensitivity_to_speed(mouse_speed).unwrap();
            FixedI32::<U16>::checked_from_num(mouse_speed).unwrap()
        };

        #[cfg(not(target_os = "windows"))]
        let p_mouse_factor = FixedI32::<U16>::from_num(3.5);
        #[cfg(target_os = "windows")]
        let p_mouse_factor = TestMouse::physical_mouse_factor();

        #[cfg(not(target_os = "windows"))]
        let v_pointer_factor = {
            let screen_update_rate = FixedI32::<U16>::from_num(DEFAULT_SCREEN_UPDATE_RATE);
            let screen_resolution = FixedI32::<U16>::from_num(DEFAULT_SCREEN_RESOLUTION);
            screen_update_rate.saturating_div(screen_resolution)
        };
        #[cfg(target_os = "windows")]
        let v_pointer_factor = TestMouse::virtual_pointer_factor();

        #[cfg(not(target_os = "windows"))]
        let smooth_mouse_curve = [
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([0x00, 0x00, 0x64, 0x00]), // 0.43
                FixedI32::from_le_bytes([0x00, 0x00, 0x96, 0x00]), // 1.25
                FixedI32::from_le_bytes([0x00, 0x00, 0xC8, 0x00]), // 3.86
                FixedI32::from_le_bytes([0x00, 0x00, 0xFA, 0x00]), // 40.0
            ],
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([0xFD, 0x11, 0x01, 0x00]), // 1.07027
                FixedI32::from_le_bytes([0x00, 0x24, 0x04, 0x00]), // 4.14062
                FixedI32::from_le_bytes([0x00, 0xFC, 0x12, 0x00]), // 18.98438
                FixedI32::from_le_bytes([0x00, 0xC0, 0xBB, 0x01]), // 443.75
            ],
        ];
        #[cfg(target_os = "windows")]
        let smooth_mouse_curve = {
            let [curve_x, curve_y] = crate::mouse_curve(true, true).unwrap();
            [curve_x.unwrap(), curve_y.unwrap()]
        };

        Self {
            ballistic,
            x_abs_fix: FixedI32::default(),
            y_abs_fix: FixedI32::default(),
            remainder_x: FixedI32::default(),
            remainder_y: FixedI32::default(),
            mouse_speed,
            p_mouse_factor,
            v_pointer_factor,
            smooth_mouse_curve,
        }
    }
}

impl TestMouse {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ballistic: bool,
        x_abs_fix: FixedI32<U16>,
        y_abs_fix: FixedI32<U16>,
        remainder_x: FixedI32<U16>,
        remainder_y: FixedI32<U16>,
        mouse_speed: FixedI32<U16>,
        p_mouse_factor: FixedI32<U16>,
        v_pointer_factor: FixedI32<U16>,
        smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
    ) -> Self {
        Self {
            ballistic,
            x_abs_fix,
            y_abs_fix,
            remainder_x,
            remainder_y,
            mouse_speed,
            p_mouse_factor,
            v_pointer_factor,
            smooth_mouse_curve,
        }
    }

    #[must_use]
    pub fn new_simple(ballistic: bool, x_start: i32, y_start: i32) -> Self {
        TestMouse {
            ballistic,
            x_abs_fix: FixedI32::<U16>::from_num(x_start),
            y_abs_fix: FixedI32::<U16>::from_num(y_start),
            ..Default::default()
        }
    }

    /// Get the scaling multipliers associated with the pointer speed slider
    /// (sensitivity)
    ///
    /// # Errors
    /// Returns an error if the provided mouse sensitivity is not a valid value
    /// ( 1 <= `mouse_sensitivity` <= 20)
    // Source https://web.archive.org/web/20241123143225/https://www.esreality.com/index.php?a=post&id=1945096
    pub fn mouse_sensitivity_to_speed(mouse_sensitivity: i32) -> Result<f32, InputError> {
        let speed = match mouse_sensitivity {
            i32::MIN..1 | 21..=i32::MAX => {
                return Err(InputError::InvalidInput(
                    "Mouse sensitivity must be between 1 and 20.",
                ));
            }
            1 => 0.1,
            2 => 0.2,
            3 => 0.3, // Guessed value
            4 => 0.4,
            5 => 0.5, // Guessed value
            6 => 0.6,
            7 => 0.7, // Guessed value
            8 => 0.8,
            9 => 0.9, // Guessed value
            10 => 1.0,
            11 => 1.1, // Guessed value
            12 => 1.2,
            13 => 1.3, // Guessed value
            14 => 1.4,
            15 => 1.5, // Guessed value
            16 => 1.6,
            17 => 1.7, // Guessed value
            18 => 1.8,
            19 => 1.9, // Guessed value
            20 => 2.0,
        };
        debug!("mouse speed: {speed}");
        Ok(speed)
    }

    /// Calculate the next location of the mouse using the smooth mouse curve
    /// and the remaining subpixels
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn calc_ballistic_location(
        x: i32,
        y: i32,
        remainder_x: FixedI32<U16>,
        remainder_y: FixedI32<U16>,
        p_mouse_factor: FixedI32<U16>,
        v_pointer_factor: FixedI32<U16>,
        mouse_speed: FixedI32<U16>,
        smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
    ) -> Option<(
        (FixedI32<U16>, FixedI32<U16>),
        (FixedI32<U16>, FixedI32<U16>),
    )> {
        if x == 0 && y == 0 {
            return Some((
                (FixedI32::<U16>::from_num(0), FixedI32::<U16>::from_num(0)),
                (remainder_x, remainder_y),
            ));
        }

        // The following list summarizes the ballistic algorithm used in Windows XP, in
        // sequence and was taken unchanged from https://web.archive.org/web/20100315061825/http://www.microsoft.com/whdc/archive/pointer-bal.mspx
        // Apparently it wasn't changed since then. The only difference is how the
        // scaling factors to physical units to virtual units are calculated

        // Summary of the Ballistic Algorithm for Windows XP
        //
        // 1. When the system is started or the mouse speed setting is changed, the
        //    translation table is recalculated and stored. The parent values are stored
        //    in the registry and in physical units that are now converted to virtual
        //    units by scaling them based on system parameters: screen refresh rate,
        //    screen resolution, default values of the mouse refresh rate (USB 125 Hz),
        //    and default mouse resolution (400 dpi). (This may change in the future to
        //    actually reflect the pointer parameters.) Then the curves are speed-scaled
        //    based on the pointer slider speed setting in the Mouse Properties dialog
        //    box (Pointer Options tab).

        let scaled_mouse_curve = Self::scale_mouse_curve(
            smooth_mouse_curve,
            p_mouse_factor,
            v_pointer_factor,
            mouse_speed,
        );

        // 2. Incoming mouse X and Y values are first converted to fixed-point 16.16
        //    format.
        let mut x_fix = FixedI32::<U16>::checked_from_num(x)?;
        let mut y_fix = FixedI32::<U16>::checked_from_num(y)?;

        // 3. The magnitude of the X and Y values is calculated and used to look up the
        //    acceleration value in the lookup table.
        let magnitude = i32::isqrt(x.checked_mul(x)? + y.checked_mul(y)?);
        // println!(" magnitude: {:?}", magnitude);
        let magnitude = FixedI32::<U16>::checked_from_num(magnitude)?;
        debug!(" magnitude: {:?}", magnitude.to_num::<f64>());

        // 4. The lookup table consists of six points (the first is [0,0]). Each point
        //    represents an inflection point, and the lookup value typically resides
        //    between the inflection points, so the acceleration multiplier value is
        //    interpolated.
        let acceleration = Self::get_acceleration(magnitude, scaled_mouse_curve)?;

        if acceleration == 0 {
            return Some((
                (FixedI32::<U16>::from_num(0), FixedI32::<U16>::from_num(0)),
                (remainder_x, remainder_y),
            ));
        }

        // 5. The remainder from the previous calculation is added to both X and Y, and
        //    then the acceleration multiplier is applied to transform the values. The
        //    remainder is stored to be added to the next incoming values, which is how
        //    subpixilation is enabled.

        // TODO: I interpret the doc to say that the multiplication should be done AFTER
        // adding the remainder. Doesnt make sense to me. Double check this
        x_fix = x_fix.checked_mul(acceleration)?;
        y_fix = y_fix.checked_mul(acceleration)?;

        x_fix = x_fix.checked_add(remainder_x)?;
        y_fix = y_fix.checked_add(remainder_y)?;

        let remainder_x = x_fix.frac();
        let remainder_y = y_fix.frac();

        // 6. The values are sent on to move the pointer.
        Some(((x_fix, y_fix), (remainder_x, remainder_y)))

        // 7. If the feature is turned off (by clearing the Enhance pointer
        //    precision check box underneath the mouse speed slider in the Mouse
        //    Properties dialog box [Pointer Options tab]), the system works as
        //    it did before without acceleration. All these functions are
        //    bypassed, and the system takes the raw mouse values and multiplies
        //    them by a scalar set based on the speed slider setting.
    }

    /// Use the smooth mouse curve to calculate the acceleration of the mouse
    #[must_use]
    pub fn get_acceleration(
        magnitude: FixedI32<U16>,
        smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
    ) -> Option<FixedI32<U16>> {
        if magnitude == FixedI32::<U16>::from_num(0) {
            return Some(FixedI32::<U16>::from_num(0));
        }

        let mut gain_factor = FixedI32::<U16>::from_num(0);

        let (mut x1, mut y1);
        let (mut x2, mut y2);

        // For each pair of points...
        for i in 0..5 {
            (x1, y1) = (smooth_mouse_curve[0][i], smooth_mouse_curve[1][i]);
            (x2, y2) = (smooth_mouse_curve[0][i + 1], smooth_mouse_curve[1][i + 1]);

            if x1 == x2 {
                continue;
            }

            let x = std::cmp::min(magnitude, x2);
            // Linear interpolation
            gain_factor += (x - x1) * ((y2 - y1) / (x2 - x1));

            // Check if x is within the range of the current segment
            if magnitude <= x2 {
                break;
            }
        }
        gain_factor /= magnitude;
        debug!(" acceleration: {:?}", gain_factor.to_num::<f64>());
        Some(gain_factor)
    }

    // TODO: Acording to the docs, the physical factor should get calculated, but
    // for some reason it is not what it is supposed to be but just 3.5 instead
    #[must_use]
    pub fn physical_mouse_factor() -> FixedI32<U16> {
        /*
        let mickey = FixedI32::<U16>::from_num(mickey);
        let bus_update_rate = FixedI32::<U16>::from_num(DEFAULT_BUS_UPDATE_RATE);
        let pointer_resolution = FixedI32::<U16>::from_num(DEFAULT_POINTER_RESOLUTION);

        let factor = bus_update_rate.checked_div(pointer_resolution)?;
        let speed = mickey.checked_mul(factor)?;*/

        FixedI32::<U16>::from_num(3.5)
    }

    #[must_use]
    #[cfg(target_os = "windows")]
    pub fn virtual_pointer_factor() -> FixedI32<U16> {
        let screen_update_rate = FixedI32::<U16>::from_num(DEFAULT_SCREEN_UPDATE_RATE);
        // TODO: Apparently this function doesn't always return the correct results
        let screen_resolution = crate::system_dpi(); // Default is 96
        debug!("DPI: {screen_resolution}");
        let screen_resolution = FixedI32::<U16>::from_num(screen_resolution);
        screen_update_rate.saturating_div(screen_resolution)
    }

    /// Scale the smooth mouse curve with the mouse speed
    #[must_use]
    pub fn scale_mouse_curve(
        smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],
        p_mouse_factor: FixedI32<U16>,
        v_pointer_factor: FixedI32<U16>,
        mouse_speed: FixedI32<U16>,
    ) -> [[FixedI32<U16>; 5]; 2] {
        let mut smooth_mouse_curve = smooth_mouse_curve;

        // Scale the X values
        for i in 0..smooth_mouse_curve[0].len() {
            smooth_mouse_curve[0][i] = smooth_mouse_curve[0][i].saturating_mul(p_mouse_factor);
        }

        // Scale the Y values
        for i in 0..smooth_mouse_curve[1].len() {
            smooth_mouse_curve[1][i] = smooth_mouse_curve[1][i]
                .saturating_mul(v_pointer_factor)
                .saturating_mul(mouse_speed);
        }

        debug!("Scaled smooth mouse: {smooth_mouse_curve:?}");
        smooth_mouse_curve
    }

    /// Predict the amount of pixels a pointer would move and update it's state
    /// (including its position)
    pub fn predict_pixel_delta(&mut self, x: i32, y: i32, coord: Coordinate) -> Option<(i32, i32)> {
        let x_fix = FixedI32::<U16>::from_num(x);
        let y_fix = FixedI32::<U16>::from_num(y);

        match coord {
            Coordinate::Abs => {
                let delta_x = self.x_abs_fix - x_fix;
                let delta_y = self.y_abs_fix - y_fix;
                self.x_abs_fix = x_fix;
                self.y_abs_fix = y_fix;
                return Some((delta_x.to_num::<i32>(), delta_y.to_num::<i32>()));
            }
            Coordinate::Rel => {
                if !self.ballistic {
                    self.x_abs_fix += x_fix;
                    self.y_abs_fix += y_fix;
                    return Some((x_fix.to_num::<i32>(), y_fix.to_num::<i32>()));
                }
            }
        }
        // Everything that follows is only done if there was a relative move and we have
        // a ballistic mouse

        let ((ballistic_x, ballistic_y), (r_x, r_y)) = Self::calc_ballistic_location(
            x,
            y,
            self.remainder_x,
            self.remainder_y,
            self.p_mouse_factor,
            self.v_pointer_factor,
            self.mouse_speed,
            self.smooth_mouse_curve,
        )?;

        self.remainder_x = r_x;
        self.remainder_y = r_y;
        self.x_abs_fix += ballistic_x;
        self.y_abs_fix += ballistic_y;
        debug!(
            "ballistic move: {}, {}",
            ballistic_x.to_num::<i32>(),
            ballistic_y.to_num::<i32>()
        );
        Some((ballistic_x.to_num::<i32>(), ballistic_y.to_num::<i32>()))
    }
}

#[cfg(test)]
mod test {
    use crate::test_mouse::TestMouse;
    use fixed::{types::extra::U16, FixedI32};

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

        let mouse_speed = FixedI32::<U16>::from_num(1.0);
        let p_mouse_factor = TestMouse::physical_mouse_factor();
        let v_pointer_factor = {
            let screen_update_rate =
                FixedI32::<U16>::from_num(crate::test_mouse::DEFAULT_SCREEN_UPDATE_RATE);
            let screen_resolution = FixedI32::<U16>::from_num(96);
            screen_update_rate.saturating_div(screen_resolution)
        };

        for curve in mouse_curves {
            for (x, correct_x) in test_case {
                println!("\n{x}");
                let ((new_x, _), _) = TestMouse::calc_ballistic_location(
                    x,
                    0,
                    remainder_x,
                    remainder_y,
                    p_mouse_factor,
                    v_pointer_factor,
                    mouse_speed,
                    curve,
                )
                .unwrap();
                assert!(i32::abs(correct_x - new_x.to_num::<i32>()) <= 1, "i: {x}");

                /*

                     fn calc_ballistic_location(
                x: i32,
                y: i32,
                remainder_x: FixedI32<U16>,
                remainder_y: FixedI32<U16>,
                p_mouse_factor: FixedI32<U16>,
                v_pointer_factor: FixedI32<U16>,
                mouse_speed: FixedI32<U16>,
                smooth_mouse_curve: [[FixedI32<U16>; 5]; 2],

                     */
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
            let acceleration = TestMouse::get_acceleration(magnitude, mouse_curves).unwrap();
            assert_eq!(acceleration.to_num::<i32>(), test.1, "x: {}", test.0);
        }
    }
}
