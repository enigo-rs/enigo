use fixed::{types::extra::U16, FixedI32};
use std::thread;
use std::time::Duration;

fn main() {
    let value: FixedI32<U16> = FixedI32::from_num(100.0);
    println!("1: {:x?}", value.to_le_bytes());
    let value: FixedI32<U16> = FixedI32::from_num(150.0);
    println!("1: {:x?}", value.to_le_bytes());
    let value: FixedI32<U16> = FixedI32::from_num(200.0);
    println!("1: {:x?}", value.to_le_bytes());
    let value: FixedI32<U16> = FixedI32::from_num(250.0);
    println!("1: {:x?}", value.to_le_bytes());

    let value: FixedI32<U16> = FixedI32::from_le_bytes([0x15, 0x6e, 0x00, 0x00]);
    println!("2: {:x?} {}", value.to_le_bytes(), value.to_num::<f32>());
}
