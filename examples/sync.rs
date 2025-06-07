/*
The Enigo struct should be created once and then reused. This example shows one
way to do that in applications that are more complex. Don't recreate and drop
the struct for each function call. It is inefficient and you will encounter bugs
due to incorrect state tracking
*/

use enigo::{Direction::Click, Enigo, Key, Keyboard, Settings};
use std::{
    sync::{LazyLock, Mutex},
    thread,
    time::Duration,
};

// Global, shared Enigo wrapped in a Mutex
static ENIGO: LazyLock<Mutex<Enigo>> =
    LazyLock::new(|| Mutex::new(Enigo::new(&Settings::default()).unwrap()));

fn press_a(from: &'static str) {
    let mut enigo = ENIGO.lock().unwrap();
    println!("[{from}] Pressing 'a'");
    enigo.key(Key::Unicode('a'), Click).unwrap();
}

fn main() {
    // Allow you to switch to another window, if you like
    println!("Starting in 2 seconds…");
    thread::sleep(Duration::from_secs(2));

    // Spawn two threads that both use the same global ENIGO
    let t1 = thread::spawn(|| press_a("Thread 1"));
    let t2 = thread::spawn(|| press_a("Thread 2"));

    t1.join().unwrap();
    t2.join().unwrap();

    println!("Done.");
}
