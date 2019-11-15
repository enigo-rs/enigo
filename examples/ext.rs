use enigo::{Enigo, Extension};

fn main() {
    let enigo = Enigo::new();
    
    println!("screen dimensions: {:?}", enigo.main_display_size());
    println!("mouse location: {:?}", enigo.mouse_location());
}