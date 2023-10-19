use std::collections::VecDeque;
use std::convert::TryInto;
use std::env;
use std::os::unix::io::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Instant;

use wayland_client::{
    protocol::{wl_pointer, wl_registry, wl_seat},
    Connection, Dispatch, EventQueue, QueueHandle,
};
use wayland_protocols_misc::{
    zwp_input_method_v2::client::{zwp_input_method_manager_v2, zwp_input_method_v2},
    zwp_virtual_keyboard_v1::client::{zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1},
};
use wayland_protocols_plasma::fake_input::client::org_kde_kwin_fake_input;
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1, zwlr_virtual_pointer_v1,
};

use super::keymap::{Bind, KeyMap, ModifierBitflag};
use crate::{
    Axis, Coordinate, Direction, InputError, InputResult, Key, KeyboardControllableNext,
    MouseButton, MouseControllableNext, NewConError,
};

pub type Keycode = u32;

pub struct Con {
    keymap: KeyMap<Keycode>,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
    virtual_keyboard: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    input_method: Option<(zwp_input_method_v2::ZwpInputMethodV2, u32)>,
    virtual_pointer: Option<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1>,
    base_time: std::time::Instant,
}

/*
if let Ok(Ok(conn)) = std::env::var("WAYLAND_DISPLAY")
        .map_err(anyhow::Error::msg)
        .map(|display_str| {
            let mut socket_path = env::var_os("XDG_RUNTIME_DIR")
                .map(Into::<PathBuf>::into)
                .ok_or(ConnectError::NoCompositor)?;
            socket_path.push(display_str);

            Ok(UnixStream::connect(socket_path).map_err(|_| ConnectError::NoCompositor)?)
        })
        .and_then(|s| s.map(|s| Connection::from_socket(s).map_err(anyhow::Error::msg)))
*/

impl Con {
    /// Tries to establish a new Wayland connection
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: &Option<String>) -> Result<Self, NewConError> {
        // Setup Wayland Connection
        let connection = match dpy_name {
            Some(dyp_name) => {
                let mut socket_path = env::var_os("XDG_RUNTIME_DIR")
                    .map(Into::<PathBuf>::into)
                    .ok_or(NewConError::EstablishCon(
                        "no XDG_RUNTIME_DIR env variable found",
                    ))?;
                socket_path.push(dyp_name);
                let stream = UnixStream::connect(socket_path)
                    .map_err(|_| NewConError::EstablishCon("unable to open unix stream"))?;
                Connection::from_socket(stream)
            }
            None => Connection::connect_to_env(),
        };

        let connection = match connection {
            Ok(connection) => connection,
            Err(e) => {
                println!("{e:?}");
                return Err(NewConError::EstablishCon(
                    "failed to connect to Wayland. Try setting 'export WAYLAND_DISPLAY=wayland-0': {e}",
                ));
            }
        };

        // Check to see if there was an error trying to connect
        if let Some(e) = connection.protocol_error() {
            println!(
                "Unknown Wayland initialization failure: {} {} {} {}",
                e.code, e.object_id, e.object_interface, e.message
            );
            return Err(NewConError::EstablishCon(
                "failed to connect to Wayland. there was a protocol error",
            ));
        }

        // Create the event queue
        let mut event_queue = connection.new_event_queue();
        // Get queue handle
        let qh = event_queue.handle();

        // Start registry
        let display = connection.display();
        display.get_registry(&qh, ());

        // Setup WaylandState and dispatch events
        let mut state = WaylandState::new();
        if event_queue.roundtrip(&mut state).is_err() {
            return Err(NewConError::EstablishCon("wayland roundtrip not possible"));
        };

        // Setup virtual keyboard
        let virtual_keyboard = if let Some(seat) = state.seat.as_ref() {
            state
                .keyboard_manager
                .as_ref()
                .map(|vk_mgr| vk_mgr.create_virtual_keyboard(seat, &qh, ()))
        } else {
            None
        };

        // Setup input method
        let input_method = if let Some(seat) = state.seat.as_ref() {
            state
                .im_manager
                .as_ref()
                .map(|im_mgr| (im_mgr.get_input_method(seat, &qh, ()), 0))
        } else {
            None
        };

        // Setup virtual pointer
        let virtual_pointer = state
            .pointer_manager
            .as_ref()
            .map(|vp_mgr| vp_mgr.create_virtual_pointer(state.seat.as_ref(), &qh, ()));

        // Try to authenticate for the KDE Fake Input protocol
        // TODO: Get this protocol to work
        if let Some(kde_input) = &state.kde_input {
            let application = "enigo".to_string();
            let reason = "enter keycodes or move the mouse".to_string();
            kde_input.authenticate(application, reason);
        }

        let base_time = Instant::now();

        let mut unused_keycodes = VecDeque::with_capacity(255 - 8 + 1); // All keycodes are unused when initialized
        for n in 8..=255 {
            unused_keycodes.push_back(n as Keycode);
        }
        let keymap = KeyMap::new(8, 255, unused_keycodes);

        Ok(Self {
            keymap,
            event_queue,
            state,
            virtual_keyboard,
            input_method,
            virtual_pointer,
            base_time,
        })
    }

    /// Get the duration since the Keymap was created
    fn get_time(&self) -> u32 {
        let duration = self.base_time.elapsed();
        let time = duration.as_millis();
        time.try_into().unwrap()
    }

    /// Check if the key is a modifier
    ///
    /// If it is a modifier, return it's bitfield.
    /// Otherwise return a None
    pub fn is_modifier(key: Key) -> Option<ModifierBitflag> {
        match key {
            Key::Shift | Key::LShift | Key::RShift => Some(Modifier::Shift as ModifierBitflag),
            Key::CapsLock => Some(Modifier::Lock as ModifierBitflag),
            Key::Control | Key::LControl | Key::RControl => {
                Some(Modifier::Control as ModifierBitflag)
            }
            Key::Alt | Key::Option => Some(Modifier::Mod1 as ModifierBitflag),
            Key::Numlock => Some(Modifier::Mod2 as ModifierBitflag),
            // Key:: => Some(Modifier::Mod3 as ModifierBitflag),
            Key::Command | Key::Super | Key::Windows | Key::Meta => {
                Some(Modifier::Mod4 as ModifierBitflag)
            }
            Key::ModeChange => Some(Modifier::Mod5 as ModifierBitflag),
            _ => None,
        }
    }

    /// Press/Release a keycode
    ///
    /// # Errors
    /// TODO
    fn send_key_event(&mut self, keycode: Keycode, direction: Direction) -> InputResult<()> {
        if let Some(vk) = &self.virtual_keyboard {
            is_alive(vk)?;
            let time = self.get_time();
            let keycode = keycode - 8; // Adjust by 8 due to the xkb/xwayland requirements

            if direction == Direction::Press || direction == Direction::Click {
                vk.key(time, keycode, 1);
                self.flush()?;
            }
            if direction == Direction::Release || direction == Direction::Click {
                vk.key(time, keycode, 0);
                self.flush()?;
            }
            return Ok(());
        }
        Err(InputError::Simulate("no way to enter key"))
    }

    /// Sends a modifier event with the updated bitflag of the modifiers to the
    /// compositor
    fn send_modifier_event(&mut self, modifiers: ModifierBitflag) -> InputResult<()> {
        if let Some(vk) = &self.virtual_keyboard {
            is_alive(vk)?;
            vk.modifiers(modifiers, 0, 0, 0);
            self.flush()?;
            return Ok(());
        }
        Err(InputError::Simulate("no way to enter modifier"))
    }

    /// Apply the current keymap
    ///
    /// # Errors
    /// TODO
    fn apply_keymap(&mut self) -> InputResult<()> {
        if let Some(vk) = &self.virtual_keyboard {
            is_alive(vk)?;
            let Ok(keymap_res) = self.keymap.regenerate() else {
                return Err(InputError::Mapping(
                    "unable to regenerate keymap".to_string(),
                ));
            };
            // Only send an updated keymap if we had to regenerate it
            // There should always be a file at this point so unwrapping is fine
            // here
            if let Some(keymap_size) = keymap_res {
                vk.keymap(1, self.keymap.file.as_ref().unwrap().as_fd(), keymap_size);
                self.flush()?;
            }
            return Ok(());
        }
        Err(InputError::Simulate("no way to apply keymap"))
    }

    /// Flush the Wayland queue
    fn flush(&self) -> InputResult<()> {
        match self.event_queue.flush() {
            Ok(()) => Ok(()),
            Err(e) => {
                println!("{e:?}");
                Err(InputError::Simulate("could not flush wayland queue"))
            }
        }
    }
}

impl Bind<Keycode> for Con {
    // Nothing to do
    // On Wayland only the whole keymap can be applied
}

pub enum Modifier {
    Shift = 0x1,
    Lock = 0x2,
    Control = 0x4,
    Mod1 = 0x8,
    Mod2 = 0x10,
    Mod3 = 0x20,
    Mod4 = 0x40,
    Mod5 = 0x80,
}

impl Drop for Con {
    // Destroy the Wayland objects we created
    fn drop(&mut self) {
        if let Some(vk) = &self.virtual_keyboard {
            vk.destroy();
        }
        if let Some((im, _)) = &self.input_method {
            im.destroy();
        }
        if let Some(vp) = &self.virtual_pointer {
            vp.destroy();
        }
        if self.flush().is_err() {
            println!("could not flush wayland queue");
        }
    }
}

/// Stores the manager for the various protocols
struct WaylandState {
    keyboard_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    im_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    pointer_manager: Option<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1>,
    kde_input: Option<org_kde_kwin_fake_input::OrgKdeKwinFakeInput>,
    seat: Option<wl_seat::WlSeat>,
    /*  output: Option<wl_output::WlOutput>,
    width: i32,
    height: i32,*/
}

impl WaylandState {
    fn new() -> Self {
        Self {
            keyboard_manager: None,
            im_manager: None,
            pointer_manager: None,
            kde_input: None,
            seat: None,
            /*  output: None,
            width: 0,
            height: 0,*/
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // When receiving events from the wl_registry, we are only interested in the
        // `global` event, which signals a new available global.
        if let wl_registry::Event::Global {
            name,
            interface,
            version: _,
        } = event
        {
            match &interface[..] {
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                    state.seat = Some(seat);
                }
                /*"wl_output" => {
                    let output = registry.bind::<wl_output::WlOutput, _, _>(name, 1, qh, ());
                    state.output = Some(output);
                }*/
                "zwp_input_method_manager_v2" => {
                    let manager = registry
                        .bind::<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, _, _>(
                            name,
                            1, // TODO: should this be 2?
                            qh,
                            (),
                        );
                    state.im_manager = Some(manager);
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    let manager = registry
                        .bind::<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                        name,
                        1,
                        qh,
                        (),
                    );
                    state.keyboard_manager = Some(manager);
                }
                "zwlr_virtual_pointer_manager_v1" => {
                    let manager = registry
                        .bind::<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, _, _>(
                        name,
                        1,
                        qh,
                        (),
                    );
                    state.pointer_manager = Some(manager);
                }
                "org_kde_kwin_fake_input" => {
                    println!("FAKE_INPUT AVAILABLE!");
                    let kde_input = registry
                        .bind::<org_kde_kwin_fake_input::OrgKdeKwinFakeInput, _, _>(
                            name,
                            1,
                            qh,
                            (),
                        );
                    state.kde_input = Some(kde_input);
                }
                s => {
                    println!("i: {s}");
                }
            }
        }
    }
}

impl Dispatch<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        event: zwp_virtual_keyboard_manager_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Received a virtual keyboard manager event {event:?}");
    }
}

impl Dispatch<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        event: zwp_virtual_keyboard_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Got a virtual keyboard event {event:?}");
    }
}

impl Dispatch<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
        event: zwp_input_method_manager_v2::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Received an input method manager event {event:?}");
    }
}
impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &zwp_input_method_v2::ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Got a virtual keyboard event {event:?}");
    }
}
impl Dispatch<org_kde_kwin_fake_input::OrgKdeKwinFakeInput, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &org_kde_kwin_fake_input::OrgKdeKwinFakeInput,
        event: org_kde_kwin_fake_input::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // This should never happen, as there are no events specified for this
        // in the protocol
        println!("Got a plasma fake input event {event:?}");
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Got a seat event {event:?}");
    }
}

/*
impl Dispatch<wl_output::WlOutput, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _output: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry {
                x,
                y,
                physical_width,
                physical_height,
                subpixel,
                make,
                model,
                transform,
            } => {
                state.width = x;
                state.height = y;
                println!("x: {x}, y: {y}, physical_width: {physical_width}, physical_height: {physical_height}, make: {make}, model: {model}");
            }
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh,
            } => {
                println!("width: {width}, height: {height}");
            }
            _ => {}
        };
    }
}*/

impl Dispatch<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
        event: zwlr_virtual_pointer_manager_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Received a virtual keyboard manager event {event:?}");
    }
}

impl Dispatch<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
        event: zwlr_virtual_pointer_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        println!("Got a virtual keyboard event {event:?}");
    }
}

impl Drop for WaylandState {
    // Destroy the manager for the protocols we used
    fn drop(&mut self) {
        if let Some(im_mgr) = self.im_manager.as_ref() {
            im_mgr.destroy();
        }
        if let Some(pointer_mgr) = self.pointer_manager.as_ref() {
            pointer_mgr.destroy();
        }
    }
}

impl KeyboardControllableNext for Con {
    fn fast_text_entry(&mut self, text: &str) -> InputResult<Option<()>> {
        if let Some((im, serial)) = &mut self.input_method {
            is_alive(im)?;
            im.commit_string(text.to_string());
            im.commit(*serial);
            *serial = serial.wrapping_add(1);
            self.flush()?;
            return Ok(Some(()));
        }
        Ok(None)
    }

    fn enter_key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        if self.keymap.make_room(&())? {
            self.apply_keymap()?;
        }
        let keycode = self.keymap.key_to_keycode(&(), key)?;

        // Apply the new keymap if there were any changes
        self.apply_keymap()?;

        // Update the status of the keymap
        let modifier = Self::is_modifier(key);

        // Send the events to the compositor
        if let Some(m) = modifier {
            if direction == Direction::Click || direction == Direction::Press {
                let modifiers = self.keymap.enter_modifier(m, Direction::Press);
                self.send_modifier_event(modifiers)?;
            }
            if direction == Direction::Click || direction == Direction::Release {
                let modifiers = self.keymap.enter_modifier(m, Direction::Release);
                self.send_modifier_event(modifiers)?;
            }
        } else {
            self.send_key_event(keycode, direction)?;
        }
        Ok(())
    }
}
impl MouseControllableNext for Con {
    fn send_mouse_button_event(
        &mut self,
        button: MouseButton,
        direction: Direction,
    ) -> InputResult<()> {
        if let Some(vp) = &self.virtual_pointer {
            // Do nothing if one of the mouse scroll buttons was released
            // Releasing one of the scroll mouse buttons has no effect
            if direction == Direction::Release {
                match button {
                    MouseButton::Left
                    | MouseButton::Right
                    | MouseButton::Back
                    | MouseButton::Forward
                    | MouseButton::Middle => {}
                    MouseButton::ScrollDown
                    | MouseButton::ScrollUp
                    | MouseButton::ScrollRight
                    | MouseButton::ScrollLeft => return Ok(()),
                }
            };

            let button = match button {
                // Taken from /linux/input-event-codes.h
                MouseButton::Left => 0x110,
                MouseButton::Right => 0x111,
                MouseButton::Back => 0x116,
                MouseButton::Forward => 0x115,
                MouseButton::Middle => 0x112,
                MouseButton::ScrollDown => return self.mouse_scroll_event(1, Axis::Vertical),
                MouseButton::ScrollUp => return self.mouse_scroll_event(-1, Axis::Vertical),
                MouseButton::ScrollRight => return self.mouse_scroll_event(1, Axis::Horizontal),
                MouseButton::ScrollLeft => return self.mouse_scroll_event(-1, Axis::Horizontal),
            };

            if direction == Direction::Press || direction == Direction::Click {
                let time = self.get_time();
                vp.button(time, button, wl_pointer::ButtonState::Pressed);
                vp.frame(); // TODO: Check if this is needed
            }

            if direction == Direction::Release || direction == Direction::Click {
                let time = self.get_time();
                vp.button(time, button, wl_pointer::ButtonState::Released);
                vp.frame(); // TODO: Check if this is needed
            }
        }
        // TODO: Change to flush()
        match self.event_queue.roundtrip(&mut self.state) {
            Ok(_) => Ok(()),
            Err(_) => Err(InputError::Simulate("The roundtrip on Wayland failed")),
        }
    }

    fn send_motion_notify_event(
        &mut self,
        x: i32,
        y: i32,
        coordinate: Coordinate,
    ) -> InputResult<()> {
        if let Some(vp) = &self.virtual_pointer {
            let time = self.get_time();
            match coordinate {
                Coordinate::Relative => {
                    vp.motion(time, x as f64, y as f64);
                }
                Coordinate::Absolute => {
                    let Ok(x) = x.try_into() else {
                        return Err(InputError::InvalidInput(
                            "the absolute coordinates cannot be negative",
                        ));
                    };
                    let Ok(y) = y.try_into() else {
                        return Err(InputError::InvalidInput(
                            "the absolute coordinates cannot be negative",
                        ));
                    };
                    vp.motion_absolute(
                        time,
                        x,
                        y,
                        u32::MAX, // TODO: Check what would be the correct value here
                        u32::MAX, // TODO: Check what would be the correct value here
                    );
                }
            }
            vp.frame(); // TODO: Check if this is needed
        }
        // TODO: Change to flush()
        match self.event_queue.roundtrip(&mut self.state) {
            Ok(_) => Ok(()),
            Err(_) => Err(InputError::Simulate("The roundtrip on Wayland failed")),
        }
    }

    fn mouse_scroll_event(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        if let Some(vp) = &self.virtual_pointer {
            // TODO: Check what the value of length should be
            // TODO: Check if it would be better to use .axis_discrete here
            let time = self.get_time();
            let axis = match axis {
                Axis::Horizontal => wl_pointer::Axis::HorizontalScroll,
                Axis::Vertical => wl_pointer::Axis::VerticalScroll,
            };
            vp.axis(time, axis, length.into());
            vp.frame(); // TODO: Check if this is needed
        }
        // TODO: Change to flush()
        match self.event_queue.roundtrip(&mut self.state) {
            Ok(_) => Ok(()),
            Err(_) => Err(InputError::Simulate("The roundtrip on Wayland failed")),
        }
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        // TODO Implement this
        println!("You tried to get the dimensions of the main display. I don't know how this is possible under Wayland. Let me know if there is a new protocol");
        Err(InputError::Simulate("Not implemented yet"))
    }

    fn mouse_loc(&self) -> InputResult<(i32, i32)> {
        // TODO Implement this
        println!("You tried to get the mouse location. I don't know how this is possible under Wayland. Let me know if there is a new protocol");
        Err(InputError::Simulate("Not implemented yet"))
    }
}

fn is_alive<P: wayland_client::Proxy>(proxy: &P) -> InputResult<()> {
    if proxy.is_alive() {
        Ok(())
    } else {
        Err(InputError::Simulate("wayland proxy is dead"))
    }
}
