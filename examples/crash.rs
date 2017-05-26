/*
 * This thing crashes Enigo.
 * Obviously, this is a bug, and is being worked on.
 * It's here for us to experiment around to see why it crashes and
 * how to fix it.
 *
 * Cheers!
 */
extern crate enigo;
use enigo::{Enigo, KeyboardControllable, Key};
use std::{thread, time};

fn main() {
    let mut enigo = Enigo::new();

    //enigo.key_sequence("ä#+ -> hello world ... 𝕊");
    enigo.key_sequence("aaa𝕊");
    println!("woot m9");
    enigo.key_click(Key::RETURN);
    println!("m9 woot");
}
