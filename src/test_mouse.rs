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
        println!("mouse speed: {speed}");
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

        let magnitude = (x_fix.checked_mul(x_fix)? + y_fix.checked_mul(y_fix)?).sqrt();
        // println!(" magnitude: {:?}", magnitude);
        println!(" magnitude: {:?}", magnitude.to_num::<f64>());

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
        println!(" acceleration: {:?}", gain_factor.to_num::<f64>());
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
        println!("DPI: {screen_resolution}");
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
        println!(
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

        let mouse_curve_ci = [
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([21, 110, 0, 0]),          // 1.50502
                FixedI32::from_le_bytes([0x00, 64, 1, 0]),         // 4.375
                FixedI32::from_le_bytes([41, 220, 3, 0]),          // 13.51
                FixedI32::from_le_bytes([0, 0, 40, 0]),            // 140
            ],
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([253, 17, 1, 0]),          // 0.83614
                FixedI32::from_le_bytes([0, 36, 4, 0]),            // 3.23486
                FixedI32::from_le_bytes([0, 252, 18, 0]),          // 14.83154
                FixedI32::from_le_bytes([0, 192, 187, 1]),         // 346.67969
            ],
        ];

        let coordinates_ci = vec![
            (0, 0),
            (1, 0),
            (2, 1),
            (3, 2),
            (4, 2),
            (5, 3),
            (6, 5),
            (7, 6),
            (8, 7),
            (9, 9),
            (10, 10),
            (11, 11),
            (12, 12),
            (13, 14),
            (14, 15),
            (15, 18),
            (16, 21),
            (17, 23),
            (18, 26),
            (19, 29),
            (20, 32),
            (21, 34),
            (22, 37),
            (23, 40),
            (24, 42),
            (25, 45),
            (26, 48),
            (27, 50),
            (28, 53),
            (29, 56),
            (30, 58),
            (31, 61),
            (32, 64),
            (33, 66),
            (34, 69),
            (35, 72),
            (36, 75),
            (37, 77),
            (38, 80),
            (39, 83),
            (40, 85),
            (41, 88),
            (42, 91),
            (43, 93),
            (44, 96),
            (45, 99),
            (46, 101),
            (47, 104),
            (48, 107),
            (49, 109),
            (50, 112),
            (51, 115),
            (52, 118),
            (53, 120),
            (54, 123),
            (55, 126),
            (56, 128),
            (57, 131),
            (58, 134),
            (59, 136),
            (60, 139),
            (61, 142),
            (62, 144),
            (63, 147),
            (64, 150),
            (65, 152),
            (66, 155),
            (67, 158),
            (68, 160),
            (69, 163),
            (70, 166),
            (71, 169),
            (72, 171),
            (73, 174),
            (74, 177),
            (75, 179),
            (76, 182),
            (77, 185),
            (78, 187),
            (79, 190),
            (80, 193),
            (81, 195),
            (82, 198),
            (83, 201),
            (84, 203),
            (85, 206),
            (86, 209),
            (87, 212),
            (88, 214),
            (89, 217),
            (90, 220),
            (91, 222),
            (92, 225),
            (93, 228),
            (94, 230),
            (95, 233),
            (96, 236),
            (97, 238),
            (98, 241),
            (99, 244),
            (100, 246),
            (101, 249),
            (102, 252),
            (103, 255),
            (104, 257),
            (105, 260),
            (106, 263),
            (107, 265),
            (108, 268),
            (109, 271),
            (110, 273),
            (111, 276),
            (112, 279),
            (113, 281),
            (114, 284),
            (115, 287),
            (116, 289),
            (117, 292),
            (118, 295),
            (119, 297),
            (120, 300),
            (121, 303),
            (122, 306),
            (123, 308),
            (124, 311),
            (125, 314),
            (126, 316),
            (127, 319),
            (128, 322),
            (129, 324),
            (130, 327),
            (131, 330),
            (132, 332),
            (133, 335),
            (134, 338),
            (135, 340),
            (136, 343),
            (137, 346),
            (138, 349),
            (139, 351),
            (140, 354),
            (141, 357),
            (142, 359),
            (143, 362),
            (144, 365),
            (145, 367),
            (146, 370),
            (147, 373),
            (148, 375),
            (149, 378),
            (150, 381),
            (151, 383),
            (152, 386),
            (153, 389),
            (154, 392),
            (155, 394),
            (156, 397),
            (157, 400),
            (158, 402),
            (159, 405),
            (160, 408),
            (161, 410),
            (162, 413),
            (163, 416),
            (164, 418),
            (165, 421),
            (166, 424),
            (167, 426),
            (168, 429),
            (169, 432),
            (170, 435),
            (171, 437),
            (172, 440),
            (173, 443),
            (174, 445),
            (175, 448),
            (176, 451),
            (177, 453),
            (178, 456),
            (179, 459),
            (180, 461),
            (181, 464),
            (182, 467),
            (183, 469),
            (184, 472),
            (185, 475),
            (186, 477),
            (187, 480),
            (188, 483),
            (189, 486),
            (190, 488),
            (191, 491),
            (192, 494),
            (193, 496),
            (194, 499),
            (195, 502),
            (196, 504),
            (197, 507),
            (198, 510),
            (199, 512),
            (200, 515),
            (201, 518),
            (202, 520),
            (203, 523),
            (204, 526),
            (205, 529),
            (206, 531),
            (207, 534),
            (208, 537),
            (209, 539),
            (210, 542),
            (211, 545),
            (212, 547),
            (213, 550),
            (214, 553),
            (215, 555),
            (216, 558),
            (217, 561),
            (218, 563),
            (219, 566),
            (220, 569),
            (221, 572),
            (222, 574),
            (223, 577),
            (224, 580),
            (225, 582),
            (226, 585),
            (227, 588),
            (228, 590),
            (229, 593),
            (230, 596),
            (231, 598),
            (232, 601),
            (233, 604),
            (234, 606),
            (235, 609),
            (236, 612),
            (237, 614),
            (238, 617),
            (239, 620),
            (240, 623),
            (241, 625),
            (242, 628),
            (243, 631),
            (244, 633),
            (245, 636),
            (246, 639),
            (247, 641),
            (248, 644),
            (249, 647),
            (250, 649),
            (251, 652),
            (252, 655),
            (253, 657),
            (254, 660),
            (255, 663),
            (256, 666),
            (257, 668),
            (258, 671),
            (259, 674),
            (260, 676),
            (261, 679),
            (262, 682),
            (263, 684),
            (264, 687),
            (265, 690),
            (266, 692),
            (267, 695),
            (268, 698),
            (269, 700),
            (270, 703),
            (271, 706),
            (272, 709),
            (273, 711),
            (274, 714),
            (275, 717),
            (276, 719),
            (277, 722),
            (278, 725),
            (279, 727),
            (280, 730),
            (281, 733),
            (282, 735),
            (283, 738),
            (284, 741),
            (285, 743),
            (286, 746),
            (287, 749),
            (288, 752),
            (289, 754),
            (290, 757),
            (291, 760),
            (292, 762),
            (293, 765),
            (294, 768),
            (295, 770),
            (296, 773),
            (297, 776),
            (298, 778),
            (299, 781),
            (300, 784),
            (301, 786),
            (302, 789),
            (303, 792),
            (304, 794),
            (305, 797),
            (306, 800),
            (307, 803),
            (308, 805),
            (309, 808),
            (310, 811),
            (311, 813),
            (312, 816),
            (313, 819),
            (314, 821),
            (315, 824),
            (316, 827),
            (317, 829),
            (318, 832),
            (319, 835),
            (320, 837),
            (321, 840),
            (322, 843),
            (323, 846),
            (324, 848),
            (325, 851),
            (326, 854),
            (327, 856),
            (328, 859),
            (329, 862),
            (330, 864),
            (331, 867),
            (332, 870),
            (333, 872),
            (334, 875),
            (335, 878),
            (336, 880),
            (337, 883),
            (338, 886),
            (339, 889),
            (340, 891),
            (341, 894),
            (342, 897),
            (343, 899),
            (344, 902),
            (345, 905),
            (346, 907),
            (347, 910),
            (348, 913),
            (349, 915),
            (350, 918),
            (351, 921),
            (352, 923),
            (353, 926),
            (354, 929),
            (355, 931),
            (356, 934),
            (357, 937),
            (358, 940),
            (359, 942),
            (360, 945),
            (361, 948),
            (362, 950),
            (363, 953),
            (364, 956),
            (365, 958),
            (366, 961),
            (367, 964),
            (368, 966),
            (369, 969),
            (370, 972),
            (371, 974),
            (372, 977),
            (373, 980),
            (374, 983),
            (375, 985),
            (376, 988),
            (377, 991),
            (378, 993),
            (379, 996),
            (380, 999),
            (381, 1001),
            (382, 1004),
            (383, 1007),
            (384, 1009),
            (385, 1012),
            (386, 1015),
            (387, 1017),
            (388, 1020),
            (389, 1023),
            (390, 1026),
            (391, 1028),
            (392, 1031),
            (393, 1034),
            (394, 1036),
            (395, 1039),
            (396, 1042),
            (397, 1044),
            (398, 1047),
            (399, 1050),
            (400, 1052),
            (401, 1055),
            (402, 1058),
            (403, 1060),
            (404, 1063),
            (405, 1066),
            (406, 1069),
            (407, 1071),
            (408, 1074),
            (409, 1077),
            (410, 1079),
            (411, 1082),
            (412, 1085),
            (413, 1087),
            (414, 1090),
            (415, 1093),
            (416, 1095),
            (417, 1098),
            (418, 1101),
            (419, 1103),
            (420, 1106),
            (421, 1109),
            (422, 1111),
            (423, 1114),
            (424, 1117),
            (425, 1120),
            (426, 1122),
            (427, 1125),
            (428, 1128),
            (429, 1130),
            (430, 1133),
            (431, 1136),
            (432, 1138),
            (433, 1141),
            (434, 1144),
            (435, 1146),
            (436, 1149),
            (437, 1152),
            (438, 1154),
            (439, 1157),
            (440, 1160),
            (441, 1163),
            (442, 1165),
            (443, 1168),
            (444, 1171),
            (445, 1173),
            (446, 1176),
            (447, 1179),
            (448, 1181),
            (449, 1184),
            (450, 1187),
            (451, 1189),
            (452, 1192),
            (453, 1195),
            (454, 1197),
            (455, 1200),
            (456, 1203),
            (457, 1206),
            (458, 1208),
            (459, 1211),
            (460, 1214),
            (461, 1216),
            (462, 1219),
            (463, 1222),
            (464, 1224),
            (465, 1227),
            (466, 1230),
            (467, 1232),
            (468, 1235),
            (469, 1238),
            (470, 1240),
            (471, 1243),
            (472, 1246),
            (473, 1248),
            (474, 1251),
            (475, 1254),
            (476, 1257),
            (477, 1259),
            (478, 1262),
            (479, 1265),
            (480, 1267),
            (481, 1270),
            (482, 1273),
            (483, 1275),
            (484, 1278),
            (485, 1281),
            (486, 1283),
            (487, 1286),
            (488, 1289),
            (489, 1291),
            (490, 1294),
            (491, 1297),
            (492, 1300),
            (493, 1302),
            (494, 1305),
            (495, 1308),
            (496, 1310),
            (497, 1313),
            (498, 1316),
            (499, 1318),
            (500, 1321),
            (501, 1324),
            (502, 1326),
            (503, 1329),
            (504, 1332),
            (505, 1334),
            (506, 1337),
            (507, 1340),
            (508, 1343),
            (509, 1345),
            (510, 1348),
            (511, 1351),
            (512, 1353),
            (513, 1356),
            (514, 1359),
            (515, 1361),
            (516, 1364),
            (517, 1367),
            (518, 1369),
            (519, 1372),
            (520, 1375),
            (521, 1377),
            (522, 1380),
            (523, 1383),
            (524, 1386),
            (525, 1388),
            (526, 1391),
            (527, 1394),
            (528, 1396),
            (529, 1399),
            (530, 1402),
            (531, 1404),
            (532, 1407),
            (533, 1410),
            (534, 1412),
            (535, 1415),
            (536, 1418),
            (537, 1420),
            (538, 1423),
            (539, 1426),
            (540, 1428),
            (541, 1431),
            (542, 1434),
            (543, 1437),
            (544, 1439),
            (545, 1442),
            (546, 1445),
            (547, 1447),
            (548, 1450),
            (549, 1453),
            (550, 1455),
            (551, 1458),
            (552, 1461),
            (553, 1463),
            (554, 1466),
            (555, 1469),
            (556, 1471),
            (557, 1474),
            (558, 1477),
            (559, 1480),
            (560, 1482),
            (561, 1485),
            (562, 1488),
            (563, 1490),
            (564, 1493),
            (565, 1496),
            (566, 1498),
            (567, 1501),
            (568, 1504),
            (569, 1506),
            (570, 1509),
            (571, 1512),
            (572, 1514),
            (573, 1517),
            (574, 1520),
            (575, 1523),
            (576, 1525),
            (577, 1528),
            (578, 1531),
            (579, 1533),
            (580, 1536),
            (581, 1539),
            (582, 1541),
            (583, 1544),
            (584, 1547),
            (585, 1549),
            (586, 1552),
            (587, 1555),
            (588, 1557),
            (589, 1560),
            (590, 1563),
            (591, 1565),
            (592, 1568),
            (593, 1571),
            (594, 1574),
            (594, 1574),
            (595, 1576),
            (596, 1579),
            (597, 1582),
            (598, 1584),
            (599, 1587),
            (600, 1590),
            (601, 1592),
            (602, 1595),
            (603, 1598),
            (604, 1600),
            (605, 1603),
            (606, 1606),
            (607, 1608),
            (608, 1611),
            (609, 1614),
            (610, 1617),
            (611, 1619),
            (612, 1622),
            (613, 1625),
            (614, 1627),
            (615, 1630),
            (616, 1633),
            (617, 1635),
            (618, 1638),
            (619, 1641),
            (620, 1643),
            (621, 1646),
            (622, 1649),
            (623, 1651),
            (624, 1654),
            (625, 1657),
            (626, 1660),
            (627, 1662),
            (628, 1665),
            (629, 1668),
            (630, 1670),
            (631, 1673),
            (632, 1676),
            (633, 1678),
            (634, 1681),
            (635, 1684),
            (636, 1686),
            (637, 1689),
            (638, 1692),
            (639, 1694),
            (640, 1697),
            (641, 1700),
            (642, 1703),
            (643, 1705),
            (644, 1708),
            (645, 1711),
            (646, 1713),
            (647, 1716),
            (648, 1719),
            (649, 1721),
            (650, 1724),
            (651, 1727),
            (652, 1729),
            (653, 1732),
            (654, 1735),
            (655, 1737),
            (656, 1740),
            (657, 1743),
            (658, 1745),
            (659, 1748),
            (660, 1751),
            (661, 1754),
            (662, 1756),
            (663, 1759),
            (664, 1762),
            (665, 1764),
            (666, 1767),
            (667, 1770),
            (668, 1772),
            (669, 1775),
            (670, 1778),
            (671, 1780),
            (672, 1783),
            (673, 1786),
            (674, 1788),
            (675, 1791),
            (676, 1794),
            (677, 1797),
            (678, 1799),
            (679, 1802),
            (680, 1805),
            (681, 1807),
            (682, 1810),
            (683, 1813),
            (684, 1815),
            (685, 1818),
            (686, 1821),
            (687, 1823),
            (688, 1826),
            (689, 1829),
            (690, 1831),
            (691, 1834),
            (692, 1837),
            (693, 1840),
            (694, 1842),
            (695, 1845),
            (696, 1848),
            (697, 1850),
            (698, 1853),
            (699, 1856),
            (700, 1858),
            (701, 1861),
            (702, 1864),
            (703, 1866),
            (704, 1869),
            (705, 1872),
            (706, 1874),
            (707, 1877),
            (708, 1880),
            (709, 1882),
            (710, 1885),
            (711, 1888),
            (712, 1891),
            (713, 1893),
            (714, 1896),
            (715, 1899),
            (716, 1901),
            (717, 1904),
            (718, 1907),
            (719, 1909),
            (720, 1912),
            (721, 1915),
            (722, 1917),
            (723, 1920),
            (724, 1923),
            (725, 1925),
            (726, 1928),
            (727, 1931),
            (728, 1934),
            (729, 1936),
            (730, 1939),
            (731, 1942),
            (732, 1944),
            (733, 1947),
            (734, 1950),
            (735, 1952),
            (736, 1955),
            (737, 1958),
            (738, 1960),
            (739, 1963),
            (740, 1966),
            (741, 1968),
            (742, 1971),
            (743, 1974),
            (744, 1977),
            (745, 1979),
            (746, 1982),
            (747, 1985),
            (748, 1987),
            (749, 1990),
            (750, 1993),
            (751, 1995),
            (752, 1998),
            (753, 2001),
            (754, 2003),
            (755, 2006),
            (756, 2009),
            (757, 2011),
            (758, 2014),
            (759, 2017),
            (760, 2019),
            (761, 2022),
            (762, 2025),
            (763, 2028),
            (764, 2030),
            (765, 2033),
            (766, 2036),
            (767, 2038),
            (768, 2041),
            (769, 2044),
            (770, 2046),
            (771, 2049),
            (772, 2052),
            (773, 2054),
            (774, 2057),
            (775, 2060),
            (776, 2062),
            (777, 2065),
            (778, 2068),
            (779, 2071),
            (780, 2073),
            (781, 2076),
            (782, 2079),
            (783, 2081),
            (784, 2084),
            (785, 2087),
            (786, 2089),
            (787, 2092),
            (788, 2095),
            (789, 2097),
            (790, 2100),
            (791, 2103),
            (792, 2105),
            (793, 2108),
            (794, 2111),
            (795, 2114),
            (796, 2116),
            (797, 2119),
            (798, 2122),
            (799, 2124),
            (800, 2127),
            (801, 2130),
            (802, 2132),
            (803, 2135),
            (804, 2138),
            (805, 2140),
            (806, 2143),
            (807, 2146),
            (808, 2148),
            (809, 2151),
            (810, 2154),
            (811, 2157),
            (812, 2159),
            (813, 2162),
            (814, 2165),
            (815, 2167),
            (816, 2170),
            (817, 2173),
            (818, 2175),
            (819, 2178),
            (820, 2181),
            (821, 2183),
            (822, 2186),
            (823, 2189),
            (824, 0),
            (825, 2194),
            (826, 2197),
            (827, 2199),
            (828, 2202),
            (829, 2205),
            (830, 2208),
            (831, 2210),
            (832, 2213),
            (833, 2216),
            (834, 2218),
            (835, 2221),
            (836, 2224),
            (837, 2226),
            (838, 2229),
            (839, 2232),
            (840, 2234),
            (841, 2237),
            (842, 2240),
            (843, 2242),
            (844, 2245),
            (845, 2248),
            (846, 2251),
            (847, 2253),
            (848, 2256),
            (849, 2259),
            (850, 2261),
            (851, 2264),
            (852, 2267),
            (853, 2269),
            (854, 2272),
            (855, 2275),
            (856, 2277),
            (857, 2280),
            (858, 2283),
            (859, 2285),
            (860, 2288),
            (861, 2291),
            (862, 2294),
            (863, 2296),
            (864, 2299),
            (865, 2302),
            (866, 2304),
            (867, 2307),
            (868, 2310),
            (869, 2312),
            (870, 2315),
            (871, 2318),
            (872, 2320),
            (873, 2323),
            (874, 2326),
            (875, 2328),
            (876, 2331),
            (877, 2334),
            (878, 2337),
            (879, 2339),
            (880, 2342),
            (881, 2345),
            (882, 2347),
            (883, 2350),
            (884, 2353),
            (885, 2355),
            (886, 2358),
            (887, 2361),
            (888, 2363),
            (889, 2366),
            (890, 2369),
            (891, 2371),
            (892, 2374),
            (893, 2377),
            (894, 2379),
            (895, 2382),
            (896, 2385),
            (897, 2388),
            (898, 2390),
            (899, 2393),
            (900, 2396),
            (901, 2398),
            (902, 2401),
            (903, 2404),
            (904, 2406),
            (905, 2409),
            (906, 2412),
            (907, 2414),
            (908, 2417),
            (909, 2420),
            (910, 2422),
            (911, 2425),
            (912, 2428),
            (913, 2431),
            (914, 2433),
            (915, 2436),
            (916, 2439),
            (917, 2441),
            (918, 2444),
            (919, 2447),
            (920, 2449),
            (921, 2452),
            (922, 2455),
            (923, 2457),
            (924, 2460),
            (925, 2463),
            (926, 2465),
            (927, 2468),
            (928, 2471),
            (929, 2474),
            (930, 2476),
            (931, 2479),
            (932, 2482),
            (933, 2484),
            (934, 2487),
            (935, 2490),
            (936, 2492),
            (937, 2495),
            (938, 2498),
            (939, 2500),
            (940, 2503),
            (941, 2506),
            (942, 2508),
            (943, 2511),
            (944, 2514),
            (945, 2516),
            (946, 2519),
            (947, 2522),
            (948, 2525),
            (949, 2527),
            (950, 2530),
            (951, 2533),
            (952, 2535),
            (953, 2538),
            (954, 2541),
            (955, 2543),
            (956, 2546),
            (957, 2549),
            (958, 2551),
            (959, 2554),
            (960, 2557),
            (961, 2559),
        ];

        /*
        let mouse_curve_extreme = [
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
        let coordinates_extreme = vec![
            (1, 0),
            (120, 6),
            (350, 19),
            (430, 10),
            (530, 0),
            (640, 12),
            (700, 19),
            (835, 4),
        ]; */

        let tests = vec![
            // (mouse_curve_extreme, coordinates_extreme),
            (mouse_curve_ci, coordinates_ci),
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

        for (curve, test_moves) in tests {
            println!("curve in test {curve:?}");
            for (x, correct_x) in test_moves {
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
                println!("{correct_x}, {}", new_x.to_num::<i32>());
                assert!(i32::abs(correct_x - new_x.to_num::<i32>()) <= 1, "i: {x}");
            }
        }
    }

    #[test]
    fn unit_acceleration() {
        const DEFAULT_SCREEN_UPDATE_RATE: i32 = 75; // in HZ
        const DEFAULT_SCREEN_RESOLUTION: i32 = 96; // in DPI

        /*
        let mouse_curves = [
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([0x15, 0x6E, 0x00, 0x00]), // 1.50502
                FixedI32::from_le_bytes([0x00, 0x40, 0x01, 0x00]), // 4.375
                FixedI32::from_le_bytes([0x29, 0xDC, 0x03, 0x00]), // 13.51
                FixedI32::from_le_bytes([0x00, 0x00, 0x28, 0x00]), // 140
            ],
            [
                FixedI32::from_le_bytes([0x00, 0x00, 0x00, 0x00]), // 0.0
                FixedI32::from_le_bytes([0xFD, 0x11, 0x01, 0x00]), // 0.83614
                FixedI32::from_le_bytes([0x00, 0x24, 0x04, 0x00]), // 3.23486
                FixedI32::from_le_bytes([0x00, 0xFC, 0x12, 0x00]), // 14.83154
                FixedI32::from_le_bytes([0x00, 0xC0, 0xBB, 0x01]), // 346.67969
            ],
        ]; */

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
