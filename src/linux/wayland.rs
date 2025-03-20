use std::{
    collections::VecDeque,
    convert::TryInto as _,
    env,
    num::Wrapping,
    os::unix::{io::AsFd, net::UnixStream},
    path::PathBuf,
    time::Instant,
};

use log::{debug, error, trace, warn};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy as _, QueueHandle, WEnum,
    protocol::{
        wl_keyboard::{self, WlKeyboard},
        wl_output::{self, Mode, WlOutput},
        wl_pointer::{self, WlPointer},
        wl_registry,
        wl_seat::{self, Capability},
    },
};
use wayland_protocols_misc::{
    zwp_input_method_v2::client::{zwp_input_method_manager_v2, zwp_input_method_v2},
    zwp_virtual_keyboard_v1::client::{zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1, zwlr_virtual_pointer_v1,
};

use super::keymap::{Bind, KeyMap};
use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse,
    NewConError, keycodes::Modifier, keycodes::ModifierBitflag,
};

pub type Keycode = u32;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
struct OutputInfo {
    width: i32,
    height: i32,
    transform: bool,
}

pub struct Con {
    keymap: KeyMap<Keycode>,
    event_queue: EventQueue<WaylandState>,
    state: WaylandState,
    base_time: std::time::Instant,
}

impl Con {
    /// Tries to establish a new Wayland connection
    ///
    /// # Errors
    /// TODO
    pub fn new(dpy_name: Option<&str>) -> Result<Self, NewConError> {
        // Setup Wayland connection
        let connection = Self::setup_connection(dpy_name)?;

        // Check to see if there was an error trying to connect
        if let Some(e) = connection.protocol_error() {
            error!(
                "unknown wayland initialization failure: {} {} {} {}",
                e.code, e.object_id, e.object_interface, e.message
            );
            return Err(NewConError::EstablishCon(
                "failed to connect to wayland. there was a protocol error",
            ));
        }

        let mut state = WaylandState::default();

        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();

        // Start registry
        let display = connection.display();
        let _ = display.get_registry(&qh, ()); // TODO: Check if we can drop the registry here

        // Receive the list of available globals
        event_queue
            .roundtrip(&mut state)
            .map_err(|_| NewConError::EstablishCon("Wayland roundtrip failed"))?;

        // Tell the compositor which globals the client wants to bind to
        event_queue
            .roundtrip(&mut state)
            .map_err(|_| NewConError::EstablishCon("Wayland roundtrip failed"))?;

        let keymap = KeyMap::new(
            8,
            255,
            // All keycodes are unused when initialized
            // Never use keycode 8
            // Keycode 8 is special: when converted to evdev keycodes,
            // 8 is subtracted, resulting in 0. This typically leads to no effect
            // when simulating input because keycode 0 corresponds to NoSymbol,
            // meaning it has no assigned key mapping.
            (9..=255).collect::<VecDeque<Keycode>>(),
            0,
            Vec::new(),
        );

        let mut connection = Self {
            keymap,
            event_queue,
            state,
            base_time: Instant::now(),
        };

        connection.init_protocols()?;
        connection.check_available_protocols()?;

        connection
            .apply_keymap()
            .map_err(|_| NewConError::EstablishCon("Unable to apply the keymap"))?;

        Ok(connection)
    }

    // Helper function for setting up the Wayland connection
    fn setup_connection(dyp_name: Option<&str>) -> Result<Connection, NewConError> {
        let connection = if let Some(dyp_name) = dyp_name {
            debug!("\x1b[93mtrying to establish a connection to: {dyp_name}\x1b[0m");
            let socket_path = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).ok_or(
                NewConError::EstablishCon("Missing XDG_RUNTIME_DIR env variable"),
            )?;
            let stream = UnixStream::connect(socket_path.join(dyp_name))
                .map_err(|_| NewConError::EstablishCon("Failed to open Unix stream"))?;
            Connection::from_socket(stream)
        } else {
            debug!("\x1b[93mtrying to establish a connection to $WAYLAND_DISPLAY\x1b[0m");
            Connection::connect_to_env()
        };

        connection.map_err(|_| {
            error!("Failed to connect to Wayland. Try setting 'WAYLAND_DISPLAY=wayland-0'.");
            NewConError::EstablishCon("Wayland connection failed.")
        })
    }

    /// Try to set up all the protocols. An error is returned, if no protocol is
    /// available
    fn init_protocols(&mut self) -> Result<(), NewConError> {
        let qh = self.event_queue.handle();

        if self.state.seat.is_some() {
            self.state.input_method =
                self.state.im_manager.as_ref().map(|im_mgr| {
                    im_mgr.get_input_method(self.state.seat.as_ref().unwrap(), &qh, ())
                });

            self.state.virtual_keyboard = self.state.keyboard_manager.as_ref().map(|vk_mgr| {
                vk_mgr.create_virtual_keyboard(self.state.seat.as_ref().unwrap(), &qh, ())
            });
        }

        // Setup virtual pointer
        self.state.virtual_pointer = self
            .state
            .pointer_manager
            .as_ref()
            .map(|vp_mgr| vp_mgr.create_virtual_pointer(self.state.seat.as_ref(), &qh, ()));

        // Wait for Activate response if the input_method was created
        // Wait for KeyMap response if virtual_keyboard was created
        if self.state.input_method.is_some() {
            self.event_queue
                .roundtrip(&mut self.state)
                .map_err(|_| NewConError::EstablishCon("Wayland roundtrip failed"))?;
        }
        Ok(())
    }

    fn check_available_protocols(&self) -> Result<(), NewConError> {
        debug!(
            "protocols available\nvirtual_keyboard: {}\ninput_method: {}\nvirtual_pointer: {}",
            self.state.virtual_keyboard.is_some(),
            self.state.input_method.is_some(),
            self.state.virtual_pointer.is_some(),
        );

        if self.state.virtual_keyboard.is_none()
            && self.state.input_method.is_none()
            && self.state.virtual_pointer.is_none()
        {
            return Err(NewConError::EstablishCon(
                "no protocol available to simulate input",
            ));
        }
        Ok(())
    }

    /// Get the duration since the Keymap was created
    fn get_time(&self) -> u32 {
        let duration = self.base_time.elapsed();
        let time = duration.as_millis();
        time.try_into().unwrap_or(u32::MAX)
    }

    /// Press/Release a keycode
    ///
    /// # Errors
    /// TODO
    fn send_key_event(&mut self, keycode: Keycode, direction: Direction) -> InputResult<()> {
        let vk = self
            .state
            .virtual_keyboard
            .as_ref()
            .ok_or(InputError::Simulate("no way to enter key"))?;
        is_alive(vk)?;
        let time = self.get_time();
        let keycode = keycode - 8; // Adjust by 8 due to the xkb/xwayland requirements

        if direction == Direction::Press || direction == Direction::Click {
            trace!("vk.key({time}, {keycode}, 1)");
            vk.key(time, keycode, 1);

            self.flush()?;
        }
        if direction == Direction::Release || direction == Direction::Click {
            trace!("vk.key({time}, {keycode}, 0)");
            vk.key(time, keycode, 0);

            self.flush()?;
        }
        Ok(())
    }

    /// Sends a modifier event with the updated bitflag of the modifiers to the
    /// compositor
    fn send_modifier_event(&mut self, modifiers: ModifierBitflag) -> InputResult<()> {
        // Retrieve virtual keyboard or return an error early if None
        let vk = self
            .state
            .virtual_keyboard
            .as_ref()
            .ok_or(InputError::Simulate("no way to enter key"))?;

        // Check if virtual keyboard is still alive
        is_alive(vk)?;

        // Log the modifier event
        trace!("vk.modifiers({modifiers}, 0, 0, 0)");

        // Send the modifier event
        vk.modifiers(modifiers, 0, 0, 0);

        self.flush()?;

        Ok(())
    }

    /// Apply the current keymap
    ///
    /// # Errors
    /// TODO
    fn apply_keymap(&mut self) -> InputResult<()> {
        trace!("apply_keymap(&mut self)");
        let vk = self
            .state
            .virtual_keyboard
            .as_ref()
            .ok_or(InputError::Simulate("no way to apply keymap"))?;
        is_alive(vk)?;

        // Regenerate keymap and handle failure
        let keymap_size = self
            .keymap
            .regenerate()
            .map_err(|_| InputError::Mapping("unable to regenerate keymap".to_string()))?;

        // Early return if the keymap was not changed because we only send an updated
        // keymap if we had to regenerate it
        let Some(size) = keymap_size else {
            return Ok(());
        };

        trace!("update wayland keymap");

        let keymap_file = self.keymap.keymap_mapping.file.as_ref().unwrap(); // Safe here, assuming file is always present
        vk.keymap(1, keymap_file.as_fd(), size);

        debug!("wait for response after keymap call");
        self.event_queue
            .roundtrip(&mut self.state)
            .map_err(|_| InputError::Simulate("Wayland roundtrip failed"))?;

        Ok(())
    }

    fn raw(&mut self, keycode: Keycode, direction: Direction) -> InputResult<()> {
        // Apply the new keymap if there were any changes
        self.apply_keymap()?;

        // Send the key event and update keymap state
        // This is important to avoid unmapping held keys
        self.send_key_event(keycode, direction)?;
        self.keymap.key(keycode, direction);

        Ok(())
    }

    /// Flush the Wayland queue
    fn flush(&self) -> InputResult<()> {
        self.event_queue.flush().map_err(|e| {
            error!("{e:?}");
            InputError::Simulate("could not flush Wayland queue")
        })?;
        trace!("flushed event queue");
        Ok(())
    }
}

impl Bind<Keycode> for Con {
    // Nothing to do
    // On Wayland only the whole keymap can be applied
}

impl Drop for Con {
    // Destroy the Wayland objects we created
    fn drop(&mut self) {
        if let Some(vk) = self.state.virtual_keyboard.take() {
            vk.destroy();
        }
        if let Some(im) = self.state.input_method.take() {
            im.destroy();
        }
        if let Some(vp) = self.state.virtual_pointer.take() {
            vp.destroy();
        }

        if self.flush().is_err() {
            error!("could not flush wayland queue");
        }
        trace!("wayland objects were destroyed");

        let _ = self.event_queue.roundtrip(&mut self.state);
    }
}

#[derive(Clone, Debug, Default)]
/// Stores the manager for the various protocols
struct WaylandState {
    // interface name, global id, version
    globals: Vec<(String, u32, u32)>,
    outputs: Vec<(WlOutput, OutputInfo)>,
    keyboard_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    virtual_keyboard: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    im_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    input_method: Option<zwp_input_method_v2::ZwpInputMethodV2>,
    im_serial: Wrapping<u32>,
    pointer_manager: Option<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1>,
    virtual_pointer: Option<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1>,
    seat: Option<wl_seat::WlSeat>,
    seat_keyboard: Option<WlKeyboard>,
    seat_pointer: Option<WlPointer>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            // Store global to later bind to them
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                trace!("Global announced: {interface} (name: {name}, version: {version})");
                match &interface[..] {
                    "wl_seat" => {
                        let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ());
                        state.seat = Some(seat);
                    }
                    "wl_output" => {
                        let wl_output =
                            registry.bind::<wl_output::WlOutput, _, _>(name, version, qh, ());
                        state.outputs.push((wl_output, OutputInfo::default()));
                    }
                    "zwp_input_method_manager_v2" => {
                        let manager = registry
                            .bind::<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, _, _>(
                                name,
                                version,
                                qh,
                                (),
                            );
                        state.im_manager = Some(manager);
                    }
                    "zwp_virtual_keyboard_manager_v1" => {
                        let manager = registry
                        .bind::<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    );
                        state.keyboard_manager = Some(manager);
                    }
                    "zwlr_virtual_pointer_manager_v1" => {
                        let manager = registry
                        .bind::<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    );
                        state.pointer_manager = Some(manager);
                    }
                    _ => {}
                }
                state.globals.push((interface, name, version));
            }
            // Remove global from store when it becomes unavailable
            wl_registry::Event::GlobalRemove { name } => {
                trace!("Global removed: {name}");
                let Some((idx, (interface, name, _))) = state
                    .globals
                    .iter()
                    .enumerate()
                    .find(|(_, (_, n, _))| *n == name)
                else {
                    return;
                };

                match &interface[..] {
                    "wl_seat" => {
                        state.im_manager = None;
                        state.keyboard_manager = None;
                        state.seat = None;
                    }
                    "wl_output" => {
                        state
                            .outputs
                            .retain(|(output, _)| output.id().protocol_id() != *name);
                    }
                    "zwp_input_method_manager_v2" => {
                        state.im_manager = None;
                    }
                    "zwp_virtual_keyboard_manager_v1" => {
                        state.keyboard_manager = None;
                    }
                    "zwlr_virtual_pointer_manager_v1" => {
                        state.pointer_manager = None;
                    }
                    _ => {}
                }
                state.globals.remove(idx);
            }
            ev => warn!("Received unknown Wayland event: {ev:?}"),
        }
    }
}

impl Dispatch<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
        event: zwp_virtual_keyboard_manager_v1::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Received a virtual keyboard manager event {event:?}");
    }
}

impl Dispatch<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
        event: zwp_virtual_keyboard_v1::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Got a virtual keyboard event {event:?}");
    }
}

impl Dispatch<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
        event: zwp_input_method_manager_v2::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Received an input method manager event {event:?}");
    }
}
impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _vk: &zwp_input_method_v2::ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Got a input method event {event:?}");
        match event {
            zwp_input_method_v2::Event::Done => state.im_serial += Wrapping(1u32),
            zwp_input_method_v2::Event::Activate
            | zwp_input_method_v2::Event::Deactivate
            | zwp_input_method_v2::Event::SurroundingText {
                text: _,
                cursor: _,
                anchor: _,
            }
            | zwp_input_method_v2::Event::TextChangeCause { cause: _ }
            | zwp_input_method_v2::Event::ContentType {
                hint: _,
                purpose: _,
            }
            | zwp_input_method_v2::Event::Unavailable => (),
            _ => todo!(),
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        (): &(),
        _con: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        warn!("Received a seat event {event:?}");
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                let capabilities = match capabilities {
                    wayland_client::WEnum::Value(capabilities) => capabilities,
                    wayland_client::WEnum::Unknown(v) => {
                        warn!("Unknown value for the capabilities of the wl_seat: {v}");
                        return;
                    }
                };

                // Create a WlKeyboard if the seat has the capability
                if state.seat_keyboard.is_none() && capabilities.contains(Capability::Keyboard) {
                    let seat_keyboard = seat.get_keyboard(qh, ());
                    state.seat_keyboard = Some(seat_keyboard);
                }

                // Create a WlPointer if the seat has the capability
                if state.seat_pointer.is_none() && capabilities.contains(Capability::Pointer) {
                    let seat_pointer = seat.get_pointer(qh, ());
                    state.seat_pointer = Some(seat_pointer);
                }
            }
            wl_seat::Event::Name { name: _ } => {}
            _ => warn!("unknown event"),
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _seat: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Got a wl_keyboard event {event:?}");
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _seat: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Got a wl_pointer event {event:?}");
    }
}

impl Dispatch<wl_output::WlOutput, ()> for WaylandState {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry { transform, .. } => {
                // The width and height need to get switched if the transform changes them
                // TODO: Check if this really is needed
                if transform == WEnum::Value(wl_output::Transform::_90)
                    || transform == WEnum::Value(wl_output::Transform::_270)
                    || transform == WEnum::Value(wl_output::Transform::Flipped90)
                    || transform == WEnum::Value(wl_output::Transform::Flipped270)
                {
                    if let Some((_, output_data)) =
                        state.outputs.iter_mut().find(|(o, _)| o == output)
                    {
                        output_data.transform = true;
                    }
                }
            }
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh,
            } => {
                warn!("flags: {flags:?}, width: {width}, : {height}, refresh: {refresh}");
                if flags == WEnum::Value(Mode::Current) {
                    if let Some((_, output_data)) =
                        state.outputs.iter_mut().find(|(o, _)| o == output)
                    {
                        output_data.width = width;
                        output_data.height = height;
                    }
                }
            }
            ev => {
                warn!("{ev:?}");
            }
        }
    }
}

impl Dispatch<zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _manager: &zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
        event: zwlr_virtual_pointer_manager_v1::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Received a virtual keyboard manager event {event:?}");
    }
}

impl Dispatch<zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _vk: &zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
        event: zwlr_virtual_pointer_v1::Event,
        (): &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        warn!("Got a virtual keyboard event {event:?}");
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

impl Keyboard for Con {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        // Process all previous events so that the serial number is correct
        self.event_queue
            .roundtrip(&mut self.state)
            .map_err(|_| InputError::Simulate("The roundtrip on Wayland failed"))?;

        let Some(im) = self.state.input_method.as_mut() else {
            return Ok(None);
        };

        is_alive(im)?;
        trace!("fast text input with imput_method protocol");

        im.commit_string(text.to_string());
        im.commit(self.state.im_serial.0);

        self.flush()?;

        Ok(Some(()))
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        let Ok(modifier) = Modifier::try_from(key) else {
            let keycode = self.keymap.key_to_keycode(&(), key)?;
            self.raw(keycode, direction)?;
            return Ok(());
        };

        // Send the events to the compositor
        trace!("it is a modifier: {modifier:?}");
        if direction == Direction::Click || direction == Direction::Press {
            let modifiers = self
                .keymap
                .enter_modifier(modifier.bitflag(), Direction::Press);
            self.send_modifier_event(modifiers)?;
        }
        if direction == Direction::Click || direction == Direction::Release {
            let modifiers = self
                .keymap
                .enter_modifier(modifier.bitflag(), Direction::Release);
            self.send_modifier_event(modifiers)?;
        }

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        self.raw(keycode as u32, direction)
    }
}
impl Mouse for Con {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        let vp = self
            .state
            .virtual_pointer
            .as_ref()
            .ok_or(InputError::Simulate("no way to enter button"))?;

        // Do nothing if one of the mouse scroll buttons was released
        // Releasing one of the scroll mouse buttons has no effect
        if direction == Direction::Release
            && matches!(
                button,
                Button::ScrollDown | Button::ScrollUp | Button::ScrollRight | Button::ScrollLeft
            )
        {
            return Ok(());
        }

        let button = match button {
            // Taken from /linux/input-event-codes.h
            Button::Left => 0x110,
            Button::Right => 0x111,
            Button::Back => 0x116,
            Button::Forward => 0x115,
            Button::Middle => 0x112,
            Button::ScrollDown => return self.scroll(1, Axis::Vertical),
            Button::ScrollUp => return self.scroll(-1, Axis::Vertical),
            Button::ScrollRight => return self.scroll(1, Axis::Horizontal),
            Button::ScrollLeft => return self.scroll(-1, Axis::Horizontal),
        };

        if direction == Direction::Press || direction == Direction::Click {
            let time = self.get_time();
            trace!("vp.button({time}, {button}, wl_pointer::ButtonState::Pressed)");
            vp.button(time, button, wl_pointer::ButtonState::Pressed);
            vp.frame(); // TODO: Check if this is needed
        }

        if direction == Direction::Release || direction == Direction::Click {
            let time = self.get_time();
            trace!("vp.button({time}, {button}, wl_pointer::ButtonState::Released)");
            vp.button(time, button, wl_pointer::ButtonState::Released);
            vp.frame(); // TODO: Check if this is needed
        }

        self.flush()
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        let vp = self
            .state
            .virtual_pointer
            .as_ref()
            .ok_or(InputError::Simulate("no way to move the mouse"))?;

        let time = self.get_time();
        match coordinate {
            Coordinate::Rel => {
                trace!("vp.motion({time}, {x}, {y})");
                vp.motion(time, x as f64, y as f64);
            }
            Coordinate::Abs => {
                let (x_extend, y_extend) = self.main_display()?;
                let x_extend: u32 = x_extend
                    .try_into()
                    .map_err(|_| InputError::InvalidInput("x_extend cannot be negative"))?;
                let y_extend: u32 = y_extend
                    .try_into()
                    .map_err(|_| InputError::InvalidInput("y_extend cannot be negative"))?;
                let x: u32 = x.try_into().map_err(|_| {
                    InputError::InvalidInput("the absolute coordinates cannot be negative")
                })?;
                let y: u32 = y.try_into().map_err(|_| {
                    InputError::InvalidInput("the absolute coordinates cannot be negative")
                })?;

                trace!("vp.motion_absolute({time}, {x}, {y}, {x_extend}, {y_extend})");
                vp.motion_absolute(time, x, y, x_extend, y_extend);
            }
        }
        vp.frame(); // TODO: Check if this is needed

        self.flush()
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        let vp = self
            .state
            .virtual_pointer
            .as_ref()
            .ok_or(InputError::Simulate("no way to scroll"))?;

        // TODO: Check what the value of length should be
        // TODO: Check if it would be better to use .axis_discrete here
        let time = self.get_time();
        let axis = match axis {
            Axis::Horizontal => wl_pointer::Axis::HorizontalScroll,
            Axis::Vertical => wl_pointer::Axis::VerticalScroll,
        };
        trace!("vp.axis(time, axis, length.into())");
        vp.axis(time, axis, length.into());
        vp.frame(); // TODO: Check if this is needed

        self.flush()
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        // TODO: The assumption here is that the output we store in the first position
        // is the main display. This likely can be wrong
        match self.state.outputs.first() {
            // Switch width and height if the output was transformed
            Some((_, output_info)) if output_info.transform => {
                Ok((output_info.height, output_info.width))
            }
            Some((_, output_info)) => Ok((output_info.width, output_info.height)),
            None => Err(InputError::Simulate("No screens available")),
        }
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        // TODO Implement this
        error!(
            "You tried to get the mouse location. I don't know how this is possible under Wayland. Let me know if there is a new protocol"
        );
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
