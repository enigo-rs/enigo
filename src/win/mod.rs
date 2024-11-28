mod win_impl;
pub use win_impl::{
    mouse_speed, mouse_thresholds_and_acceleration, set_mouse_speed,
    set_mouse_thresholds_and_acceleration, system_dpi, Enigo, EXT,
};

#[cfg(feature = "test_mouse")]
pub use win_impl::{mouse_curve, set_mouse_curve};
