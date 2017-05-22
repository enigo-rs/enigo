extern crate enigo;

use enigo::Enigo;
use std::io::Write;

use std::{thread, time};

fn main() {
    let wait_time = time::Duration::from_millis(200);
    let mut enigo = Enigo::new();

    thread::sleep(wait_time);

    // Currently will not fail.
    #[cfg(target_os = "linux")]
    write!(enigo, "Hello World").unwrap();
}
