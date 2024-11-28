use fixed::{types::extra::U16, FixedI32};
use log::debug;

use crate::{Coordinate, InputError};

// const DEFAULT_BUS_UPDATE_RATE: i32 = 125; // in HZ
// const DEFAULT_POINTER_RESOLUTION: i32 = 400; // in mickey/inch
// const DEFAULT_SCREEN_RESOLUTION: i32 = 96; // in DPI
const DEFAULT_SCREEN_UPDATE_RATE: i32 = 75; // in HZ

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
        println!(" magnitude: {:?}", magnitude.to_num::<f64>());

        // 4. The lookup table consists of six points (the first is [0,0]). Each point
        //    represents an inflection point, and the lookup value typically resides
        //    between the inflection points, so the acceleration multiplier value is
        //    interpolated.
        let acceleration = Self::get_acceleration(magnitude, scaled_mouse_curve)?;
        println!(" acceleration: {:?}", acceleration.to_num::<f64>());

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

        println!("Scaled smooth mouse: {smooth_mouse_curve:?}");
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
        Some((ballistic_x.to_num::<i32>(), ballistic_y.to_num::<i32>()))
    }
}

mod test {

    #[test]
    fn testa() {}
}
