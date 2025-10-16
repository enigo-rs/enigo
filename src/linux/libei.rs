use ashpd::desktop::remote_desktop::RemoteDesktop;
use log::{debug, error, trace, warn};
use reis::{
    PendingRequestResult,
    ei::{self, Connection},
    handshake::HandshakeResp,
};
use std::{collections::HashMap, os::unix::net::UnixStream, time::Instant};
use xkbcommon::xkb;

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};
pub type Keycode = u32;

static INTERFACES: std::sync::LazyLock<HashMap<&'static str, u32>> =
    std::sync::LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert("ei_button", 1);
        m.insert("ei_callback", 1);
        m.insert("ei_connection", 1);
        m.insert("ei_device", 2);
        m.insert("ei_keyboard", 1);
        m.insert("ei_pingpong", 1);
        m.insert("ei_pointer", 1);
        m.insert("ei_pointer_absolute", 1);
        m.insert("ei_scroll", 1);
        m.insert("ei_seat", 1);
        m
    });

#[derive(Debug, Default, PartialEq, Clone)]
struct SeatData {
    name: Option<String>,
    capabilities: HashMap<String, u64>,
}

#[derive(Debug, Default, PartialEq, Copy, Clone)]
enum DeviceState {
    #[default]
    Paused,
    Resumed,
    Emulating,
}

#[derive(Debug, Default, PartialEq, Copy, Clone)]
struct DeviceRegion {
    offset_x: u32, // region x offset in logical pixels
    offset_y: u32, // region y offset in logical pixels
    width: u32,    // region width in logical pixels
    height: u32,   // region height in logical pixels
    scale: f32,    // the physical scale for this region
}

#[derive(Debug, Default, PartialEq, Clone)]
struct DeviceData {
    name: Option<String>,
    device_type: Option<ei::device::DeviceType>,
    interfaces: HashMap<String, reis::Object>,
    state: DeviceState,
    dimensions: Option<(u32, u32)>, // width, height
    regions: Vec<DeviceRegion>,
}

impl DeviceData {
    fn interface<T: reis::Interface>(&self) -> Option<T> {
        self.interfaces.get(T::NAME)?.clone().downcast()
    }
}

/// The main struct for handling the event emitting
#[derive(Clone)]
pub struct Con {
    // XXX best way to handle data associated with object?
    // TODO: Release seat when dropped, so compositor knows it wont be used anymore
    seats: HashMap<ei::Seat, SeatData>,
    // XXX association with seat?
    // TODO: Release device when dropped, so compositor knows it wont be used anymore
    devices: HashMap<ei::Device, DeviceData>,
    keyboards: HashMap<ei::Keyboard, xkb::Keymap>,
    /// `None` if there was no disconnect
    disconnect: Option<(ei::connection::DisconnectReason, String)>,
    sequence: u32,
    last_serial: u32,
    context: ei::Context,
    connection: Connection,
    time_created: Instant,
}

// This is safe, we have a unique pointer.
// TODO: use Unique<c_char> once stable.
unsafe impl Send for Con {}

impl Con {
    async fn open_connection() -> Result<ei::Context, NewConError> {
        use ashpd::desktop::remote_desktop::DeviceType;

        trace!("open_connection");

        match ei::Context::connect_to_env() {
            Ok(Some(context)) => {
                trace!("done open_connection after connect_to_env");
                return Ok(context);
            }
            Ok(None) => {
                debug!("Unable to find ei socket. Trying xdg desktop portal.");
            }
            Err(e) => {
                error! {"{e}"}
                return Err(NewConError::EstablishCon("error while checking ei env"));
            }
        }

        // Fallback: use portal
        let remote_desktop = RemoteDesktop::new().await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create RemoteDesktop")
        })?;
        trace!("New desktop");

        let session = remote_desktop.create_session().await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create remote desktop session")
        })?;

        remote_desktop
            .select_devices(
                &session,
                // TODO: Add DeviceType::Touchscreen once we support it in enigo
                DeviceType::Keyboard | DeviceType::Pointer,
                None, // TODO: Allow passing the restore_token via the EnigoSettings
                ashpd::desktop::PersistMode::Application, /* TODO: Allow passing the
                       * restore_token via the
                       * EnigoSettings */
            )
            .await
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to select devices")
            })?;
        trace!("new session");

        remote_desktop.start(&session, None).await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to start remote desktop session")
        })?;
        trace!("start session");

        let fd = remote_desktop.connect_to_eis(&session).await.map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to connect to EIS")
        })?;
        // fd is a raw descriptor returned by portal; construct UnixStream
        let stream = UnixStream::from(fd);
        stream
            // TODO: Check if this is a good idea
            .set_nonblocking(true)
            .map_err(|e| {
                error! {"{e}"};
                NewConError::EstablishCon("failed to set nonblocking on stream")
            })?;
        trace!("done open_connection");

        ei::Context::new(stream).map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("failed to create ei context")
        })
    }

    #[allow(unnecessary_wraps)] // The wrap is needed for the libei_tokio feature
    fn custom_block_on<F: Future>(f: F) -> Result<F::Output, NewConError> {
        #[cfg(feature = "libei_tokio")]
        if tokio::runtime::Handle::try_current().is_err() {
            return Ok(tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .map_err(|e| {
                    error! {"{e}"};
                    NewConError::EstablishCon("failed to create tokio runtime")
                })?
                .block_on(f));
        }
        Ok(futures::executor::block_on(f))
    }

    #[allow(clippy::unnecessary_wraps)]
    /// Create a new Enigo instance
    pub fn new() -> Result<Self, NewConError> {
        debug!("using libei");

        let libei_name = "enigo";

        let seats = HashMap::new();
        let devices = HashMap::new();
        let keyboards = HashMap::new();
        let disconnect = None;
        let sequence = 0;
        let time_created = Instant::now();

        // open_connection now returns Result<ei::Context, NewConError>
        let context = Self::custom_block_on(Self::open_connection())??;

        let HandshakeResp {
            connection,
            serial,
            negotiated_interfaces,
        } = reis::handshake::ei_handshake_blocking(
            &context,
            libei_name,
            ei::handshake::ContextType::Sender,
        )
        .map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("handshake failed")
        })?;

        trace!("main: handshake");

        context.flush().map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("unable to flush the libei context")
        })?;
        trace!("main: flushed");

        let mut con = Self {
            seats,
            devices,
            keyboards,
            disconnect,
            sequence,
            last_serial: serial.wrapping_add(1),
            context,
            connection,
            time_created,
        };

        con.update(libei_name).map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("unable to update the libei connection")
        })?;

        for (device, device_data) in con.devices.iter_mut().filter(|(_, device_data)| {
            device_data.device_type == Some(reis::ei::device::DeviceType::Virtual)
                && device_data.state == DeviceState::Resumed
            // TODO: Should all devices start emulating?
            // && device_data.interface::<ei::Keyboard>().is_some()
        }) {
            debug!("Start emulating");
            if !device.is_alive() {
                return Err(NewConError::EstablishCon("ei::Device is no longer alive"));
            }
            device.start_emulating(con.last_serial, con.sequence);
            con.sequence = con.sequence.wrapping_add(1);
            device_data.state = DeviceState::Emulating;
        }

        con.update(libei_name).map_err(|e| {
            error! {"{e}"};
            NewConError::EstablishCon("unable to update the libei connection")
        })?;

        Ok(con)
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, libei_name: &str) -> InputResult<()> {
        let mut had_pending_events = true;

        loop {
            debug!("update");
            if self.context.read().is_err() {
                error!("err reading");
                return Err(InputError::Simulate("Failed to update libei context"));
            }

            while let Some(result) = self.context.pending_event() {
                had_pending_events = true;
                trace!("found pending_event");

                let request = match result {
                    PendingRequestResult::Request(request) => request,
                    PendingRequestResult::ParseError(msg) => {
                        error!("parse error from libei: {msg}");
                        return Err(InputError::Simulate("failed to parse pending request"));
                    }
                    PendingRequestResult::InvalidObject(object_id) => {
                        // TODO
                        error!("invalid object with id {object_id}");
                        continue;
                    }
                };

                trace!("found request");
                match request {
                    ei::Event::Handshake(handshake, request) => match request {
                        ei::handshake::Event::HandshakeVersion { version: _ } => {
                            trace!("handshake version");
                            handshake.handshake_version(1);
                            handshake.name(libei_name);
                            handshake.context_type(ei::handshake::ContextType::Sender);
                            for (interface, version) in INTERFACES.iter() {
                                handshake.interface_version(interface, *version);
                            }
                            handshake.finish();
                        }
                        ei::handshake::Event::InterfaceVersion { name, version } => {
                            // TODO: Use the interface versions
                            trace!("Received: interface {name}, version {version}");
                        }
                        ei::handshake::Event::Connection {
                            connection: _,
                            serial,
                        } => {
                            trace!("handshake connection");
                            self.last_serial = serial;
                            self.sequence = serial;
                        }
                        _ => {
                            warn!("handshake else");
                        }
                    },
                    ei::Event::Connection(connection, request) => match request {
                        ei::connection::Event::Disconnected {
                            last_serial,
                            reason,
                            explanation,
                        } => {
                            self.seats.clear();
                            self.seats.shrink_to_fit();
                            self.devices.clear();
                            self.devices.shrink_to_fit();
                            self.keyboards.clear();
                            self.keyboards.shrink_to_fit();
                            self.disconnect = Some((reason, explanation));
                            self.sequence = 0;
                            self.last_serial = last_serial;
                        }
                        ei::connection::Event::Seat { seat } => {
                            trace!("connection seat");
                            self.seats.insert(seat, SeatData::default());
                        }
                        ei::connection::Event::InvalidObject {
                            last_serial,
                            invalid_id,
                        } => {
                            // TODO: Try to recover?
                            error!(
                                "the serial {last_serial} contained an invalid object with the id {invalid_id}"
                            );
                        }
                        ei::connection::Event::Ping { ping } => {
                            debug!("ping");
                            if !ping.is_alive() {
                                return Err(InputError::Simulate(
                                    "ei::Pingpong is no longer alive",
                                ));
                            }
                            ping.done(0);
                        }
                        _ => {
                            warn!("Unknown connection event");
                        }
                    },
                    ei::Event::Seat(seat, request) => {
                        trace!("connection seat");
                        if let Some(data) = self.seats.get_mut(&seat) {
                            match request {
                                ei::seat::Event::Destroyed { serial } => {
                                    debug!("seat was destroyed");
                                    self.seats.remove(&seat);
                                }
                                ei::seat::Event::Name { name } => {
                                    data.name = Some(name);
                                }
                                ei::seat::Event::Capability { mask, interface } => {
                                    data.capabilities.insert(interface, mask);
                                }
                                ei::seat::Event::Done => {
                                    let mut bitmask = 0;
                                    if let Some(bits) = data.capabilities.get("ei_button") {
                                        bitmask |= bits;
                                    }
                                    if let Some(bits) = data.capabilities.get("ei_keyboard") {
                                        bitmask |= bits;
                                    }
                                    if let Some(bits) = data.capabilities.get("ei_pointer") {
                                        bitmask |= bits;
                                    }
                                    if let Some(bits) = data.capabilities.get("ei_pointer_absolute")
                                    {
                                        bitmask |= bits;
                                    }
                                    if let Some(bits) = data.capabilities.get("ei_scroll") {
                                        bitmask |= bits;
                                    }
                                    if let Some(bits) = data.capabilities.get("ei_touchscreen") {
                                        bitmask |= bits;
                                    }

                                    seat.bind(bitmask);
                                    trace!("done binding to seat");
                                }
                                ei::seat::Event::Device { device } => {
                                    self.devices.insert(device, DeviceData::default());
                                }
                                _ => {
                                    warn!("Unknown seat event");
                                }
                            }
                        } else {
                            warn!("received Seat event for unknown seat");
                        }
                    }
                    ei::Event::Device(device, request) => {
                        trace!("device event");
                        if let Some(data) = self.devices.get_mut(&device) {
                            match request {
                                ei::device::Event::Destroyed { serial } => {
                                    debug!("device with serial {serial} was destroyed");
                                    self.devices.remove(&device);
                                }
                                ei::device::Event::Name { name } => {
                                    trace!("device name: {name}");
                                    data.name = Some(name);
                                }
                                ei::device::Event::DeviceType { device_type } => {
                                    trace!("device type: {device_type:?}");
                                    data.device_type = Some(device_type);
                                }
                                ei::device::Event::Dimensions { width, height } => {
                                    trace!("device dimensions: {width}, {height}");
                                    data.dimensions = Some((width, height));
                                }
                                ei::device::Event::Region {
                                    offset_x,
                                    offset_y,
                                    width,
                                    hight: height,
                                    scale,
                                } => {
                                    trace!(
                                        "device region: {offset_x}, {offset_y}, {width}, {height}, {scale}"
                                    );
                                    data.regions.push(DeviceRegion {
                                        offset_x,
                                        offset_y,
                                        width,
                                        height,
                                        scale,
                                    });
                                }
                                ei::device::Event::Interface { object } => {
                                    trace!("device interface: {}", object.interface());
                                    data.interfaces
                                        .insert(object.interface().to_string(), object);
                                }
                                ei::device::Event::Done => {
                                    trace!("device done");
                                }
                                ei::device::Event::Resumed { serial } => {
                                    debug!("device resumed serial: {serial}");
                                    self.last_serial = serial;
                                    data.state = DeviceState::Resumed;
                                }
                                ei::device::Event::Paused { serial } => {
                                    debug!("device paused serial: {serial}");
                                    self.last_serial = serial;
                                    data.state = DeviceState::Paused;
                                }
                                _ => {
                                    warn!("device else");
                                }
                            }
                        } else {
                            warn!("received Device event for unknown device");
                        }
                    }
                    ei::Event::Keyboard(keyboard, request) => {
                        trace!("keyboard event");
                        match request {
                            ei::keyboard::Event::Destroyed { serial } => {
                                debug!("keyboard was destroyed");
                                self.keyboards.remove(&keyboard);
                            }
                            ei::keyboard::Event::Keymap {
                                keymap_type,
                                size,
                                keymap,
                            } => {
                                if keymap_type != ei::keyboard::KeymapType::Xkb {
                                    error!("The keymap is of the wrong type");
                                    continue;
                                }
                                let context = xkb::Context::new(0);
                                // xkb::Keymap::new_from_fd returns Result<Option<Keymap>, _>
                                match unsafe {
                                    xkb::Keymap::new_from_fd(
                                        &context,
                                        keymap,
                                        size as _,
                                        xkb::KEYMAP_FORMAT_TEXT_V1,
                                        0,
                                    )
                                } {
                                    Ok(Some(k)) => {
                                        self.keyboards.insert(keyboard, k);
                                    }
                                    Ok(None) => {
                                        error!("xkb returned None when creating keymap");
                                        return Err(InputError::Simulate(
                                            "failed to create keymap",
                                        ));
                                    }
                                    Err(_) => {
                                        error!("xkb returned error when creating keymap");
                                        return Err(InputError::Simulate(
                                            "failed to create keymap",
                                        ));
                                    }
                                }
                            }
                            ei::keyboard::Event::Modifiers {
                                serial,
                                depressed,
                                locked,
                                latched,
                                group,
                            } => {
                                // TODO: Handle updated modifiers
                                // Notification that the EIS
                                // implementation has changed modifier states
                                // on this device. Future ei_keyboard.key
                                // requests must take the new modifier state
                                // into account.
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        warn!("else");
                    }
                }
            }

            trace!("devices: {:?}", self.devices);

            if self.context.flush().is_ok() {
                trace!("flush success");
            } else {
                error!("flush fail");
            }

            // This is needed so anything is typed
            std::thread::sleep(std::time::Duration::from_millis(10));
            trace!("update flush");
            trace!("update done");

            // Stop looking if there were no pending events
            if !had_pending_events {
                break;
            }
            had_pending_events = false;
        }
        Ok(())
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        warn!("fast text entry is not yet implemented with libei");
        // TODO: Add fast method
        Ok(None)
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        // Find a device that exposes a keyboard interface
        let (device, device_data) = self
            .devices
            .iter_mut()
            .find(|(_, device_data)| device_data.interface::<ei::Keyboard>().is_some())
            .ok_or_else(|| {
                InputError::Simulate(
                    "cannot simulate key event: no device implementing the `ei::Keyboard` \
                    interface was found on any connected device",
                )
            })?;

        // Find the first available keyboard keymap
        let (keyboard, keymap) = self.keyboards.iter().next().ok_or_else(|| {
            InputError::Simulate(
                "cannot simulate key event: no keyboard keymap available (no `ei::Keyboard` \
                    object registered in the connection)",
            )
        })?;

        // Map the Key to a keycode using the retrieved keymap
        let keycode = key_to_keycode(keymap, key).map_err(|e| {
            error! {"{e}"};
            InputError::InvalidInput(
                "failed to map the requested key to a keycode: the provided key is not mapped in \
                 the current xkb keymap",
            )
        })?;

        // Ensure the keyboard object is still alive
        if !keyboard.is_alive() {
            return Err(InputError::Simulate(
                "cannot simulate key event: the `ei::Keyboard` object is no longer alive",
            ));
        }

        // Press
        if direction == Direction::Press || direction == Direction::Click {
            keyboard.key(keycode - 8, ei::keyboard::KeyState::Press);

            // It is a client bug to send more than one key request for the same key within
            // the same ei_device.frame and the EIS implementation may ignore either or all
            // key state changes and/or disconnect the client
            // (source https://libinput.pages.freedesktop.org/libei/interfaces/ei_keyboard/index.html#ei_keyboardkey).
            // That's why we need to call frame for the press and the release
            let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?
            device.frame(self.sequence, elapsed);
            self.sequence = self.sequence.wrapping_add(1);
        }

        // Release
        if direction == Direction::Release || direction == Direction::Click {
            keyboard.key(keycode - 8, ei::keyboard::KeyState::Released);

            let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?
            device.frame(self.sequence, elapsed);
            self.sequence = self.sequence.wrapping_add(1);
        }

        self.update("enigo").map_err(|e| {
            error! {"{e}"};
            InputError::Simulate(
                "failed to update libei connection after sending key events: the update call \
                 returned an error",
            )
        })?;

        Ok(())
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        let keycode = keycode as u32;

        // Find a device that exposes a keyboard interface
        let (device, device_data) = self
            .devices
            .iter_mut()
            .find(|(_, device_data)| device_data.interface::<ei::Keyboard>().is_some())
            .ok_or_else(|| {
                InputError::Simulate(
                    "cannot simulate raw key event: no device implementing the `ei::Keyboard` \
                    interface was found on any connected device",
                )
            })?;

        // Acquire the keyboard interface object from the device data
        let keyboard = device_data.interface::<ei::Keyboard>().ok_or_else(|| {
            InputError::Simulate(
                "cannot simulate raw key event: device lost its `ei::Keyboard` interface before \
                 the request could be sent",
            )
        })?;

        if !keyboard.is_alive() {
            return Err(InputError::Simulate(
                "cannot simulate raw key event: the `ei::Keyboard` interface is no longer alive",
            ));
        }

        // Press
        if direction == Direction::Press || direction == Direction::Click {
            keyboard.key(keycode - 8, ei::keyboard::KeyState::Press);
        }

        // Release
        if direction == Direction::Release || direction == Direction::Click {
            keyboard.key(keycode - 8, ei::keyboard::KeyState::Released);
        }

        let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?

        device.frame(self.sequence, elapsed);
        self.sequence = self.sequence.wrapping_add(1);

        self.update("enigo").map_err(|e| {
            error! {"{e}"};
            InputError::Simulate(
                "failed to update libei connection after sending raw key events: the update \
                 call returned an error",
            )
        })?;

        Ok(())
    }
}

impl Mouse for Con {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        let (device, device_data) = self
            .devices
            .iter_mut()
            .find(|(_, device_data)| device_data.interface::<ei::Button>().is_some())
            .ok_or_else(|| {
                InputError::Simulate(
                    "cannot simulate button event: no device implementing the `ei::Button` \
                    interface was found on any connected device",
                )
            })?;

        // Do nothing if one of the mouse scroll buttons was released
        // Releasing one of the scroll mouse buttons has no effect
        if direction == Direction::Release {
            match button {
                Button::Left | Button::Right | Button::Back | Button::Forward | Button::Middle => {}
                Button::ScrollDown
                | Button::ScrollUp
                | Button::ScrollRight
                | Button::ScrollLeft => {
                    return Ok(());
                }
            }
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

        let vp = device_data.interface::<ei::Button>().ok_or_else(|| {
            InputError::Simulate(
                "cannot simulate button event: the device lost its `ei::Button` interface \
                 before the operation could be performed",
            )
        })?;

        if !vp.is_alive() {
            return Err(InputError::Simulate(
                "cannot simulate button event: the `ei::Button` interface is no longer alive",
            ));
        }

        if direction == Direction::Press || direction == Direction::Click {
            trace!("vp.button({button}, ei::button::ButtonState::Press)");
            vp.button(button, ei::button::ButtonState::Press);
            // self.update("enigo");
            let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?
            device.frame(self.sequence, elapsed);
            self.sequence = self.sequence.wrapping_add(1);
        }

        if direction == Direction::Release || direction == Direction::Click {
            trace!("vp.button({button}, ei::button::ButtonState::Released)");
            vp.button(button, ei::button::ButtonState::Released);
            // self.update("enigo");
            let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?
            device.frame(self.sequence, elapsed);
            self.sequence = self.sequence.wrapping_add(1);
        }

        self.update("enigo").map_err(|e| {
            error! {"{e}"};
            InputError::Simulate(
                "failed to update libei connection after sending button events: the update call \
                 returned an error",
            )
        })?;

        Ok(())
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (x as f32, y as f32);

        match coordinate {
            Coordinate::Rel => {
                trace!("vp.motion_relative({x}, {y})");
                let (device, device_data) = self
                    .devices
                    .iter()
                    .find(|(_, device_data)| device_data.interface::<ei::Pointer>().is_some())
                    .ok_or_else(|| {
                        InputError::Simulate(
                            "cannot move mouse relatively: no device implementing the `ei::Pointer` \
                             interface was found on any connected device",
                        )
                    })?;

                let vp = device_data.interface::<ei::Pointer>().ok_or_else(|| {
                    InputError::Simulate(
                        "cannot move mouse relatively: the device lost its `ei::Pointer` \
                         interface before the operation could be performed",
                    )
                })?;

                if !vp.is_alive() {
                    return Err(InputError::Simulate(
                        "cannot move mouse relatively: the `ei::Pointer` interface is no longer alive",
                    ));
                }

                vp.motion_relative(x, y);

                let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?

                device.frame(self.sequence, elapsed);
                self.sequence = self.sequence.wrapping_add(1);

                self.update("enigo").map_err(|e| {
                    error! {"{e}"};
                    InputError::Simulate(
                        "failed to update libei connection after sending relative pointer events: \
                         the update call returned an error",
                    )
                })?;
                Ok(())
            }
            Coordinate::Abs => {
                if x < 0.0 || y < 0.0 {
                    return Err(InputError::InvalidInput(
                        "the absolute coordinates cannot be negative",
                    ));
                }

                trace!("vp.motion_absolute({x}, {y})");

                // Find a device exposing the absolute pointer interface
                let (device, device_data) = self
                    .devices
                    .iter()
                    .find(|(_, device_data)| {
                        device_data.interface::<ei::PointerAbsolute>().is_some()
                    })
                    .ok_or_else(|| {
                        InputError::Simulate(
                            "cannot move mouse absolutely: no device implementing the \
                             `ei::PointerAbsolute` interface was found on any connected device",
                        )
                    })?;

                let vp = device_data.interface::<ei::PointerAbsolute>().ok_or_else(|| {
                    InputError::Simulate(
                        "cannot move mouse absolutely: the device lost its `ei::PointerAbsolute` \
                         interface before the operation could be performed",
                    )
                })?;

                if !vp.is_alive() {
                    return Err(InputError::Simulate(
                        "cannot move mouse absolutely: the `ei::PointerAbsolute` interface is no longer alive",
                    ));
                }
                vp.motion_absolute(x, y);

                let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?

                device.frame(self.sequence, elapsed);
                self.sequence = self.sequence.wrapping_add(1);

                self.update("enigo").map_err(|e| {
                    error! {"{e}"};
                    InputError::Simulate(
                        "failed to update libei connection after sending absolute pointer events: \
                         the update call returned an error",
                    )
                })?;
                Ok(())
            }
        }
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        #[allow(clippy::cast_precision_loss)]
        let length = length as f32;

        let (device, device_data) = self
            .devices
            .iter()
            .find(|(_, device_data)| device_data.interface::<ei::Scroll>().is_some())
            .ok_or_else(|| {
                InputError::Simulate(
                    "cannot scroll: no device implementing the `ei::Scroll` interface was found \
                     on any connected device",
                )
            })?;

        let (x, y) = match axis {
            Axis::Horizontal => (length, 0.0),
            Axis::Vertical => (0.0, length),
        };
        trace!("vp.scroll({x}, {y})");

        let vp = device_data.interface::<ei::Scroll>().ok_or_else(|| {
            InputError::Simulate(
                "cannot scroll: the device lost its `ei::Scroll` interface before the operation \
                 could be performed",
            )
        })?;

        if !vp.is_alive() {
            return Err(InputError::Simulate(
                "cannot scroll: the `ei::Scroll` interface is no longer alive",
            ));
        }
        vp.scroll(x, y);

        let elapsed = self.time_created.elapsed().as_secs(); // Is seconds fine?

        device.frame(self.sequence, elapsed);
        self.sequence = self.sequence.wrapping_add(1);
        self.update("enigo").map_err(|e| {
            error! {"{e}"};
            InputError::Simulate(
                "failed to update libei connection after sending scroll events: the update call \
                 returned an error",
            )
        })?;
        Ok(())
    }

    fn main_display(&self) -> InputResult<(i32, i32)> {
        // TODO Implement this
        error!(
            "You tried to get the dimensions of the main display. I don't know how this is possible under Wayland. Let me know if there is a new protocol"
        );
        Err(InputError::Simulate(
            "main_display is not implemented: Wayland does not provide a protocol to query the main display size",
        ))
    }

    fn location(&self) -> InputResult<(i32, i32)> {
        // TODO Implement this
        error!(
            "You tried to get the mouse location. I don't know how this is possible under Wayland. Let me know if there is a new protocol"
        );
        Err(InputError::Simulate(
            "location is not implemented: Wayland does not provide a protocol to query the global pointer location",
        ))
    }
}

impl Drop for Con {
    fn drop(&mut self) {
        // TODO: Is it needed to filter or can we just stop emulating on all devices??
        for (device, _) in self.devices.iter().filter(|(_, device_data)| {
            device_data.device_type == Some(reis::ei::device::DeviceType::Virtual)
                && device_data.state == DeviceState::Emulating
        }) {
            debug!("stopping emulation for device during Drop");
            device.stop_emulating(self.last_serial);
            self.last_serial = self.last_serial.wrapping_add(1);
        }
        self.connection.disconnect(); // Let the server know we voluntarily disconnected

        let _ = self.context.flush(); // Ignore the errors if the connection was
        // dropped
    }
}

fn key_to_keycode(keymap: &xkb::Keymap, key: Key) -> InputResult<Keycode> {
    let all_keycodes = keymap.min_keycode().raw()..keymap.max_keycode().raw();

    let keysym = xkb::Keysym::from(key);
    let mut keycode = None;
    'outer: for i in all_keycodes.clone() {
        for j in 0..=1 {
            let syms = keymap.key_get_syms_by_level(xkb::Keycode::new(i), 0, j);
            if syms.contains(&keysym) {
                keycode = Some(i);
                break 'outer;
            }
        }
    }
    keycode.ok_or(crate::InputError::InvalidInput("Key is not mapped"))
}
