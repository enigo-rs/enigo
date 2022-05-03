// Imports from other crates
use gdk_sys::{GdkDisplay, GdkSeat};
use glib::translate::ToGlibPtr;
use wayland_client::{
    protocol::wl_seat::WlSeat, sys::client::wl_display, Display, EventQueue, GlobalManager, Proxy,
};
use wayland_protocols::wlr::unstable::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use zwp_input_method::input_method_unstable_v2::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use zwp_virtual_keyboard::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;

// Modules
pub mod keymap;
pub mod layer_shell;
pub mod vk_service;

/// Create a type (needed for compatibility between C and Rust)
#[allow(non_camel_case_types)]
type wl_seat = libc::c_void;

/// Declares C functions that can be called
extern "C" {
    fn gdk_wayland_display_get_wl_display(display: *mut GdkDisplay) -> *mut wl_display;
    fn gdk_wayland_seat_get_wl_seat(seat: *mut GdkSeat) -> *mut wl_seat;
}

// The type declarations are not necessary but make the code easier to read
type LayerShell = wayland_client::Main<ZwlrLayerShellV1>;
type VirtualKeyboardMgr = wayland_client::Main<ZwpVirtualKeyboardManagerV1>;
type InputMethodMgr = wayland_client::Main<ZwpInputMethodManagerV2>;

/// Get the wayland display and the wayland seat
/// This contains UNSAFE code but there currently are no other ways
fn get_wl_display_seat() -> (Display, WlSeat) {
    // Get the wayland Display from the GTK wayland connection
    // This is unsafe but there are not other ways to get the wayland connection and starting a new one does not work
    let gdk_display = gdk::Display::default();
    let display_ptr = unsafe { gdk_wayland_display_get_wl_display(gdk_display.to_glib_none().0) };
    let display = unsafe { Display::from_external_display(display_ptr) };

    // Get the 'WlSeat' from the GTK wayland connection
    let gdk_seat = gdk_display.expect("No gdk_display").default_seat();
    let seat_ptr = unsafe { gdk_wayland_seat_get_wl_seat(gdk_seat.to_glib_none().0) };
    let seat = unsafe { Proxy::<WlSeat>::from_c_ptr(seat_ptr as *mut _) };
    let seat: WlSeat = WlSeat::from(seat);
    (display, seat)
}

/// Get the 'GlobalManager' and the 'EventQueue'
fn get_wl_global_mgr(display: &Display) -> (EventQueue, GlobalManager) {
    // Create the event queue
    let mut event_queue = display.create_event_queue();
    // Attach the display
    let attached_display = display.attach(event_queue.token());

    // Get the GlobalManager
    let global_mgr = GlobalManager::new(&attached_display);

    // sync_roundtrip is a special kind of dispatching for the event queue.
    // Rather than just blocking once waiting for replies, it'll block
    // in a loop until the server has signalled that it has processed and
    // replied accordingly to all requests previously sent by the client.
    //
    // In this case, this allows to be sure that after this call returns,
    // the full list of globals was received.
    event_queue
        .sync_roundtrip(
            // No global state is used
            &mut (),
            // The only object that can receive events is the WlRegistry, and the
            // GlobalManager already takes care of assigning it to a callback, so
            // no orphan events can be received at this point
            |_, _, _| unreachable!(),
        )
        .unwrap();
    (event_queue, global_mgr)
}

/// Tries to get the manager for the protocols input_method and virtual_keyboard
/// It returns 'None' if the compositor does not undestand a protocol
fn try_get_mgrs(
    global_mgr: &GlobalManager,
) -> (Option<VirtualKeyboardMgr>, Option<InputMethodMgr>) {
    let mut virtual_keyboard_option = None;
    let mut input_method_mgr_option = None;
    if let Ok(vk_mgr) = global_mgr.instantiate_exact::<ZwpVirtualKeyboardManagerV1>(1) {
        virtual_keyboard_option = Some(vk_mgr);
    } else {
        warn!("Your wayland compositor does not understand the wp_virtual_keyboard protocol. Entering any keycode will fail");
    }
    if let Ok(im_mgr) = global_mgr.instantiate_exact::<ZwpInputMethodManagerV2>(1) {
        input_method_mgr_option = Some(im_mgr);
    } else {
        warn!("Your wayland compositor does not understand the wp_virtual_keyboard protocol. Only keycodes can be entered");
    }
    (virtual_keyboard_option, input_method_mgr_option)
}

/// Tries to get the LayerShell object to create layers
/// It returns 'None' if the compositor does not undestand the layer_shell protocol
pub fn get_layer_shell() -> Option<LayerShell> {
    let (display, _) = get_wl_display_seat(); // Gets the wayland Display so that this method can be called independently from the submitter
    let (_, global_mgr) = get_wl_global_mgr(&display); // Event queue can be dropped because it was only used to find out if layer_shell is available
    let mut layer_shell_option = None;
    if let Ok(layer_shell) = global_mgr.instantiate_exact::<ZwlrLayerShellV1>(1) {
        layer_shell_option = Some(layer_shell);
    } else {
        warn!("Your wayland compositor does not understand the gtk-layer-shell protocol. The keyboard is started in a window, just like a regular application")
    }
    layer_shell_option
}

/// Initializes the wayland connection and returns the wayland objects needed to submit text and keycodes
pub fn init_wayland() -> (
    EventQueue,
    WlSeat,
    Option<VirtualKeyboardMgr>,
    Option<InputMethodMgr>,
) {
    // Get wayland display and WlSeat
    let (display, seat) = get_wl_display_seat();
    // Get the event queue and the GlobalManager
    let (event_queue, global_mgr) = get_wl_global_mgr(&display);
    // Try to get the manager for the input_method and virtual_keyboard protocol
    let (vk_mgr, im_mgr) = try_get_mgrs(&global_mgr);
    info!("Wayland connection and objects initialized");
    (event_queue, seat, vk_mgr, im_mgr)
}
