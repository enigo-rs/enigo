use ashpd::desktop::{
    CreateSessionOptions,
    remote_desktop::{ConnectToEISOptions, RemoteDesktop, SelectDevicesOptions, StartOptions},
};
use log::{debug, error, trace, warn};
use reis::{
    Interface, PendingRequestResult,
    ei::{self, Connection},
    handshake::HandshakeResp,
};
use std::{collections::HashMap, os::unix::net::UnixStream};
use xkbcommon::xkb;

use crate::{
    Axis, Button, Coordinate, Direction, InputError, InputResult, Key, Keyboard, Mouse, NewConError,
};
pub type Keycode = u32;

/// `ei_text.utf8` allows at most 254 bytes (255 including the terminating NUL).
const EI_TEXT_MAX_UTF8_LEN: usize = 254;

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
pub struct Con {
    // XXX best way to handle data associated with object?
    // TODO: Release seat when dropped, so compositor knows it wont be used anymore
    seats: HashMap<ei::Seat, SeatData>,
    // XXX association with seat?
    // TODO: Release device when dropped, so compositor knows it wont be used anymore
    devices: HashMap<ei::Device, DeviceData>,
    keyboards: HashMap<ei::Keyboard, xkb::Keymap>,
    /// `None` if there was no disconnect
    disconnect: Option<(ei::connection::DisconnectReason, Option<String>)>,
    sequence: u32,
    last_serial: u32,
    context: ei::Context,
    connection: Connection,
    restore_token: Option<String>,
}

// SAFETY: `Con` is not auto-`Send` only because `xkb::Keymap` wraps a
// non-atomic `*mut xkb_keymap`. Moving a `Con` between threads is fine as
// long as it remains the sole owner of that keymap, which it does: `Con` is
// not `Clone`, and nothing else shares the `Keymap` values stored here.
unsafe impl Send for Con {}

impl Con {
    async fn open_connection(
        restore_token: Option<&str>,
    ) -> Result<(ei::Context, Option<String>), NewConError> {
        use ashpd::desktop::remote_desktop::DeviceType;

        trace!("open_connection");

        match ei::Context::connect_to_env() {
            Ok(Some(context)) => {
                trace!("done open_connection after connect_to_env");
                return Ok((context, None));
            }
            Ok(None) => {
                debug!("Unable to find ei socket. Trying xdg desktop portal.");
            }
            Err(e) => {
                error!("{e}");
                return Err(NewConError::EstablishCon("error while checking ei env"));
            }
        }

        // Fallback: use portal
        let remote_desktop = RemoteDesktop::new().await.map_err(|e| {
            error!("{e}");
            NewConError::EstablishCon("failed to create RemoteDesktop")
        })?;
        trace!("New desktop");

        let session = remote_desktop
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to create remote desktop session")
            })?;

        let mut options = SelectDevicesOptions::default()
            .set_devices(DeviceType::Keyboard | DeviceType::Pointer)
            .set_persist_mode(ashpd::desktop::PersistMode::Application);
        if let Some(restore_token) = restore_token {
            options = options.set_restore_token(restore_token);
        }

        remote_desktop
            .select_devices(&session, options)
            .await
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to select devices")
            })?;
        trace!("new session");

        let restore_token = remote_desktop
            .start(&session, None, StartOptions::default())
            .await
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to start remote desktop session")
            })?
            .response()
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to get remote desktop session response")
            })?
            .restore_token()
            .map(str::to_owned);
        trace!("start session");

        let fd = remote_desktop
            .connect_to_eis(&session, ConnectToEISOptions::default())
            .await
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to connect to EIS")
            })?;
        // fd is a raw descriptor returned by portal; construct UnixStream
        let stream = UnixStream::from(fd);
        stream
            // TODO: Check if this is a good idea
            .set_nonblocking(true)
            .map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("failed to set nonblocking on stream")
            })?;
        trace!("done open_connection");

        let context = ei::Context::new(stream).map_err(|e| {
            error!("{e}");
            NewConError::EstablishCon("failed to create ei context")
        })?;
        Ok((context, restore_token))
    }

    #[allow(clippy::unnecessary_wraps)] // The wrap is needed for the tokio feature
    fn custom_block_on<F: Future>(f: F) -> Result<F::Output, NewConError> {
        #[cfg(feature = "tokio")]
        if tokio::runtime::Handle::try_current().is_err() {
            return Ok(tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .map_err(|e| {
                    error!("{e}");
                    NewConError::EstablishCon("failed to create tokio runtime")
                })?
                .block_on(f));
        }
        Ok(futures::executor::block_on(f))
    }

    /// Create a new Enigo instance
    pub fn new(restore_token: Option<&str>) -> Result<Self, NewConError> {
        debug!("using libei");

        let libei_name = "enigo";

        let seats = HashMap::new();
        let devices = HashMap::new();
        let keyboards = HashMap::new();
        let disconnect = None;
        let sequence = 0;

        let (context, restore_token) =
            Self::custom_block_on(Self::open_connection(restore_token))??;

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
            error!("{e}");
            NewConError::EstablishCon("handshake failed")
        })?;

        trace!("main: handshake");

        context.flush().map_err(|e| {
            error!("{e}");
            NewConError::EstablishCon("unable to flush the libei context")
        })?;
        trace!("main: flushed");

        let mut con = Self {
            seats,
            devices,
            keyboards,
            disconnect,
            sequence,
            // The serial of the ei_handshake.connection event is the start of the EIS
            // implementation's serial number sequence
            last_serial: serial,
            context,
            connection,
            restore_token,
        };

        // The socket is non-blocking, so a single update may return before the EIS
        // implementation has advertised seats and devices. Poll until at least one
        // device is resumed (or we time out)
        let mut saw_resumed_device = false;
        for _ in 0..50 {
            con.update().map_err(|e| {
                error!("{e}");
                NewConError::EstablishCon("unable to update the libei connection")
            })?;
            if con
                .devices
                .values()
                .any(|data| data.state == DeviceState::Resumed)
            {
                saw_resumed_device = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !saw_resumed_device {
            return Err(NewConError::EstablishCon(
                "timed out waiting for the EIS implementation to resume a device",
            ));
        }

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

        con.update().map_err(|e| {
            error!("{e}");
            NewConError::EstablishCon("unable to update the libei connection")
        })?;

        Ok(con)
    }

    /// Returns the restore token from the portal session, if one was issued.
    /// Callers should save this token and pass it via `Settings::restore_token`
    /// on the next connection to skip the permission dialog.
    #[must_use]
    pub fn restore_token(&self) -> Option<String> {
        self.restore_token.clone()
    }

    /// Return an error if the EIS implementation has disconnected us. Without
    /// this check, later send paths would fail with a misleading "no device
    /// implementing …" error after `Disconnected` cleared the device maps
    fn ensure_connected(&self) -> InputResult<()> {
        if let Some((reason, explanation)) = &self.disconnect {
            error!("EIS disconnected the client: reason={reason:?}, explanation={explanation:?}");
            return Err(InputError::Simulate(
                "cannot simulate input: the EIS implementation disconnected the client",
            ));
        }
        Ok(())
    }

    /// Find a device exposing interface `T` and ensure it is emulating.
    /// Returns `Ok(None)` if no such device exists
    fn device_with<T: Interface>(&mut self) -> InputResult<Option<(ei::Device, T)>> {
        let Some((device, iface, data)) = self.devices.iter_mut().find_map(|(device, data)| {
            let iface = data.interface::<T>()?;
            Some((device.clone(), iface, data))
        }) else {
            return Ok(None);
        };

        if !device.is_alive() {
            return Err(InputError::Simulate(
                "cannot simulate input: the `ei::Device` is no longer alive",
            ));
        }
        if !iface.as_object().is_alive() {
            return Err(InputError::Simulate(
                "cannot simulate input: the device interface is no longer alive",
            ));
        }

        ensure_emulating(&device, data, &mut self.sequence, self.last_serial)?;
        Ok(Some((device, iface)))
    }

    /// Drop any seat/device/keyboard/interface we track for `object_id`.
    /// Returns whether anything was removed.
    fn drop_object_by_id(&mut self, object_id: u64) -> bool {
        // Object IDs are unique, so seat/device can return as soon as they match.
        // A keyboard id may appear in both `devices.interfaces` and `keyboards`, so
        // those two maps are cleaned together before returning.
        if self.seats.keys().any(|seat| seat.as_object().id() == object_id) {
            self.seats
                .retain(|seat, _| seat.as_object().id() != object_id);
            return true;
        }

        if let Some(device) = self
            .devices
            .keys()
            .find(|device| device.as_object().id() == object_id)
            .cloned()
        {
            if let Some(data) = self.devices.remove(&device) {
                if let Some(keyboard) = data.interface::<ei::Keyboard>() {
                    self.keyboards.remove(&keyboard);
                }
            }
            return true;
        }

        let had_interface = self
            .devices
            .values()
            .any(|data| data.interfaces.values().any(|object| object.id() == object_id));
        let had_keyboard = self
            .keyboards
            .keys()
            .any(|keyboard| keyboard.as_object().id() == object_id);
        if !had_interface && !had_keyboard {
            return false;
        }

        for data in self.devices.values_mut() {
            data.interfaces
                .retain(|_, object| object.id() != object_id);
        }
        self.keyboards
            .retain(|keyboard, _| keyboard.as_object().id() != object_id);
        true
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self) -> InputResult<()> {
        self.ensure_connected()?;

        loop {
            debug!("update");

            // Flush first so queued requests reach the EIS implementation before we
            // look for replies. Previously a sleep was used to paper over sending
            // before flushing
            if let Err(e) = self.context.flush() {
                error!("{e}");
                return Err(InputError::Simulate("Failed to flush libei context"));
            }

            // WouldBlock returns Ok(0); only real I/O failures are Err
            if let Err(e) = self.context.read() {
                error!("err reading: {e}");
                return Err(InputError::Simulate("Failed to update libei context"));
            }

            let mut had_pending_events = false;
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
                        // Event for an object already gone from the wire map. Drop any
                        // leftover local state; if nothing matched this is usually a late
                        // event after Destroyed and can be ignored.
                        error!("invalid object with id {object_id}");
                        if self.drop_object_by_id(object_id) {
                            debug!("dropped local state for invalid object {object_id}");
                        }
                        continue;
                    }
                };

                trace!("found request");
                match request {
                    // Handshake events are handled by `reis::handshake` before
                    // `update` is ever called
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
                            error!(
                                "the serial {last_serial} contained an invalid object with the id {invalid_id}"
                            );
                            self.last_serial = last_serial;
                            // Unwind any local state for the object the EIS rejected
                            let _ = self.drop_object_by_id(invalid_id);
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
                        trace!("seat event");
                        if let Some(data) = self.seats.get_mut(&seat) {
                            match request {
                                ei::seat::Event::Destroyed { serial } => {
                                    debug!("seat was destroyed");
                                    self.last_serial = serial;
                                    self.seats.remove(&seat);
                                }
                                ei::seat::Event::Name { name } => {
                                    data.name = Some(name);
                                }
                                ei::seat::Event::Capability { mask, interface } => {
                                    data.capabilities.insert(interface, mask);
                                }
                                ei::seat::Event::Done => {
                                    // Bind every capability the seat advertised. Unknown
                                    // interfaces are ignored when their events arrive
                                    let bitmask =
                                        data.capabilities.values().fold(0u64, |a, b| a | b);
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
                                    self.last_serial = serial;
                                    // Drop associated keymaps too; the compositor may tear
                                    // down the device without a separate keyboard.destroyed
                                    if let Some(data) = self.devices.remove(&device) {
                                        if let Some(keyboard) = data.interface::<ei::Keyboard>() {
                                            self.keyboards.remove(&keyboard);
                                        }
                                    }
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
                                    // libei 1.6 / ei_device v3: servers that advertise v3 wait for
                                    // ready() before sending resumed.
                                    if device.version() >= 3 {
                                        device.ready();
                                        trace!("sent device ready");
                                    }
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
                                    warn!("unknown device event");
                                }
                            }
                        } else {
                            warn!("received device event for unknown device");
                        }
                    }
                    ei::Event::Keyboard(keyboard, request) => {
                        trace!("keyboard event");
                        match request {
                            ei::keyboard::Event::Destroyed { serial } => {
                                debug!("keyboard was destroyed");
                                self.last_serial = serial;
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
                                self.last_serial = serial;
                                // TODO: Handle updated modifiers
                                // Notification that the EIS
                                // implementation has changed modifier states
                                // on this device. Future ei_keyboard.key
                                // requests must take the new modifier state
                                // into account.
                            }
                            _ => {
                                warn!("unknown keyboard event");
                            }
                        }
                    }
                    ei::Event::Text(text, request) => {
                        // The keysym/utf8 events are only sent to receiver contexts. As a
                        // sender, we only expect the destructor.
                        match request {
                            ei::text::Event::Destroyed { serial } => {
                                debug!("text interface was destroyed");
                                self.last_serial = serial;
                                // Remove the stale object so fast_text falls back to per-key
                                // entry instead of erroring
                                for data in self.devices.values_mut() {
                                    data.interfaces
                                        .retain(|_, object| *object != *text.as_object());
                                }
                            }
                            _ => {
                                warn!("unknown text event");
                            }
                        }
                    }
                    _ => {
                        warn!("unhandled libei event");
                    }
                }
            }

            trace!("devices: {:?}", self.devices);

            // If a Disconnected event was just processed, report it instead of
            // failing on the closed socket in the next iteration
            self.ensure_connected()?;

            // No more events available: return immediately on the hot path. If we
            // handled events (and may have queued replies such as ping.done), loop
            // to flush and drain follow-ups without sleeping
            if !had_pending_events {
                break;
            }
        }
        Ok(())
    }
}

impl Keyboard for Con {
    fn fast_text(&mut self, text: &str) -> InputResult<Option<()>> {
        self.ensure_connected()?;

        if text.contains('\0') {
            return Err(InputError::InvalidInput(
                "the text to enter contained a NULL byte ('\\0'), which is not allowed",
            ));
        }

        // libei 1.6+: prefer ei_text when the seat advertises it
        let Some((device, text_iface)) = self.device_with::<ei::Text>()? else {
            debug!("fast text entry not available: no device with ei_text");
            return Ok(None);
        };

        for chunk in utf8_byte_chunks(text, EI_TEXT_MAX_UTF8_LEN) {
            trace!("ei_text.utf8({chunk:?})");
            text_iface.utf8(chunk);
            // At most one utf8 request per frame
            device.frame(self.last_serial, now_monotonic_micros());
        }

        self.update()?;
        Ok(Some(()))
    }

    fn key(&mut self, key: Key, direction: Direction) -> InputResult<()> {
        self.ensure_connected()?;

        // Prefer `ei_text.keysym` (libei 1.6+): it enters the keysym directly and is
        // independent of the keymap, so even keys that are not mapped on the current
        // layout can be simulated
        if let Some((device, text_iface)) = self.device_with::<ei::Text>()? {
            let keysym = xkb::Keysym::from(key).raw();

            if direction == Direction::Press || direction == Direction::Click {
                trace!("ei_text.keysym({keysym:#x}, Press)");
                text_iface.keysym(keysym, ei::keyboard::KeyState::Press);
                // Press and release of the same keysym must be in separate frames
                device.frame(self.last_serial, now_monotonic_micros());
            }
            if direction == Direction::Release || direction == Direction::Click {
                trace!("ei_text.keysym({keysym:#x}, Released)");
                text_iface.keysym(keysym, ei::keyboard::KeyState::Released);
                device.frame(self.last_serial, now_monotonic_micros());
            }

            return self.update();
        }

        debug!("no device with ei_text: falling back to ei_keyboard and the keymap");

        let (device, keyboard) =
            self.device_with::<ei::Keyboard>()?
                .ok_or(InputError::Simulate(
                    "no connected device implements the required interface",
                ))?;

        // Use the keymap of the keyboard the key event will be sent on
        let keymap = self.keyboards.get(&keyboard).ok_or({
            InputError::Simulate(
                "cannot simulate key event: no keymap was received for the keyboard",
            )
        })?;
        let keycode = key_to_keycode(keymap, key).map_err(|e| {
            error!("{e}");
            InputError::InvalidInput(
                "failed to map the requested key to a keycode: the provided key is not mapped in \
                 the current xkb keymap",
            )
        })?;
        // XKB keycodes in the keymap are X11-style; ei_keyboard.key expects evdev
        // codes (X11 keycode − 8)
        let keycode = keycode.checked_sub(8).ok_or(InputError::InvalidInput(
            "the mapped keycode must be at least 8 (X11 keycode offset)",
        ))?;

        if direction == Direction::Press || direction == Direction::Click {
            keyboard.key(keycode, ei::keyboard::KeyState::Press);
            // Press and release of the same key must be in separate frames
            device.frame(self.last_serial, now_monotonic_micros());
        }
        if direction == Direction::Release || direction == Direction::Click {
            keyboard.key(keycode, ei::keyboard::KeyState::Released);
            device.frame(self.last_serial, now_monotonic_micros());
        }

        self.update()
    }

    fn raw(&mut self, keycode: u16, direction: Direction) -> InputResult<()> {
        self.ensure_connected()?;

        // The keycode is an X11 keycode, which is offset by 8 from the evdev keycode
        // that ei_keyboard.key expects. Subtracting from a keycode < 8 would
        // underflow
        let keycode = u32::from(keycode).checked_sub(8).ok_or({
            InputError::InvalidInput("the keycode must be at least 8 (X11 keycode offset)")
        })?;

        let (device, keyboard) =
            self.device_with::<ei::Keyboard>()?
                .ok_or(InputError::Simulate(
                    "no connected device implements the required interface",
                ))?;

        if direction == Direction::Press || direction == Direction::Click {
            keyboard.key(keycode, ei::keyboard::KeyState::Press);
            // Press and release of the same key must be in separate frames
            device.frame(self.last_serial, now_monotonic_micros());
        }
        if direction == Direction::Release || direction == Direction::Click {
            keyboard.key(keycode, ei::keyboard::KeyState::Released);
            device.frame(self.last_serial, now_monotonic_micros());
        }

        self.update()
    }
}

impl Mouse for Con {
    fn button(&mut self, button: Button, direction: Direction) -> InputResult<()> {
        self.ensure_connected()?;

        // Releasing a scroll "button" has no effect
        if direction == Direction::Release {
            match button {
                Button::ScrollDown
                | Button::ScrollUp
                | Button::ScrollRight
                | Button::ScrollLeft => return Ok(()),
                Button::Left | Button::Right | Button::Back | Button::Forward | Button::Middle => {}
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

        let (device, vp) = self
            .device_with::<ei::Button>()?
            .ok_or(InputError::Simulate(
                "no connected device implements the required interface",
            ))?;

        if direction == Direction::Press || direction == Direction::Click {
            trace!("vp.button({button}, ei::button::ButtonState::Press)");
            vp.button(button, ei::button::ButtonState::Press);
            // Press and release of the same button must be in separate frames
            device.frame(self.last_serial, now_monotonic_micros());
        }
        if direction == Direction::Release || direction == Direction::Click {
            trace!("vp.button({button}, ei::button::ButtonState::Released)");
            vp.button(button, ei::button::ButtonState::Released);
            device.frame(self.last_serial, now_monotonic_micros());
        }

        self.update()
    }

    fn move_mouse(&mut self, x: i32, y: i32, coordinate: Coordinate) -> InputResult<()> {
        self.ensure_connected()?;

        #[allow(clippy::cast_precision_loss)]
        let (x, y) = (x as f32, y as f32);

        match coordinate {
            Coordinate::Rel => {
                trace!("vp.motion_relative({x}, {y})");
                let (device, vp) =
                    self.device_with::<ei::Pointer>()?
                        .ok_or(InputError::Simulate(
                            "no connected device implements the required interface",
                        ))?;
                vp.motion_relative(x, y);
                device.frame(self.last_serial, now_monotonic_micros());
                self.update()
            }
            Coordinate::Abs => {
                if x < 0.0 || y < 0.0 {
                    return Err(InputError::InvalidInput(
                        "the absolute coordinates cannot be negative",
                    ));
                }

                trace!("vp.motion_absolute({x}, {y})");
                let (device, vp) =
                    self.device_with::<ei::PointerAbsolute>()?
                        .ok_or(InputError::Simulate(
                            "no connected device implements the required interface",
                        ))?;
                vp.motion_absolute(x, y);
                device.frame(self.last_serial, now_monotonic_micros());
                self.update()
            }
        }
    }

    fn scroll(&mut self, length: i32, axis: Axis) -> InputResult<()> {
        self.ensure_connected()?;

        // Wheel-style scrolling uses ei_scroll.scroll_discrete, where one logical
        // scroll unit (one wheel click) is 120
        let delta = length.checked_mul(120).ok_or(InputError::InvalidInput(
            "scroll length is too large for ei_scroll.scroll_discrete",
        ))?;
        let (x, y) = match axis {
            Axis::Horizontal => (delta, 0),
            Axis::Vertical => (0, delta),
        };
        trace!("vp.scroll_discrete({x}, {y})");

        let (device, vp) = self
            .device_with::<ei::Scroll>()?
            .ok_or(InputError::Simulate(
                "no connected device implements the required interface",
            ))?;
        vp.scroll_discrete(x, y);
        device.frame(self.last_serial, now_monotonic_micros());
        self.update()
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
        }
        self.connection.disconnect(); // Let the server know we voluntarily disconnected

        let _ = self.context.flush(); // Ignore the errors if the connection was
        // dropped
    }
}

fn key_to_keycode(keymap: &xkb::Keymap, key: Key) -> InputResult<Keycode> {
    let keysym = xkb::Keysym::from(key);
    (keymap.min_keycode().raw()..keymap.max_keycode().raw())
        .find(|&keycode| {
            (0..=1).any(|level| {
                keymap
                    .key_get_syms_by_level(xkb::Keycode::new(keycode), 0, level)
                    .contains(&keysym)
            })
        })
        .ok_or(InputError::InvalidInput("Key is not mapped"))
}

/// Timestamp for `ei_device.frame` in microseconds of `CLOCK_MONOTONIC`, as
/// required by the protocol
#[allow(clippy::cast_sign_loss)]
fn now_monotonic_micros() -> u64 {
    // SAFETY: An all-zero timespec is a valid value
    let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
    // SAFETY: The pointer to the timespec is valid for the duration of the call
    let ret = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut ts) };
    // Cannot fail: CLOCK_MONOTONIC is supported on all unix platforms
    debug_assert_eq!(ret, 0, "clock_gettime(CLOCK_MONOTONIC) failed");
    (ts.tv_sec as u64) * 1_000_000 + (ts.tv_nsec as u64) / 1_000
}

/// Ensure the device is in the `Emulating` state before sending events.
/// The EIS implementation may silently discard events that are sent to a
/// device that is not emulating.
///
/// If the device was resumed but emulation was not started yet (e.g because it
/// was paused and resumed again mid-session), `start_emulating` is sent. It is
/// not a protocol violation to do so, because the state is reset whenever the
/// device gets paused
fn ensure_emulating(
    device: &ei::Device,
    data: &mut DeviceData,
    sequence: &mut u32,
    last_serial: u32,
) -> InputResult<()> {
    match data.state {
        DeviceState::Emulating => Ok(()),
        DeviceState::Resumed => {
            debug!("start emulating before sending events");
            device.start_emulating(last_serial, *sequence);
            *sequence = sequence.wrapping_add(1);
            data.state = DeviceState::Emulating;
            Ok(())
        }
        // Only the EIS implementation can resume a paused device. Requesting
        // emulation on a device that is not resumed is a client bug, so
        // failing is all we can do here
        DeviceState::Paused => Err(InputError::Simulate(
            "cannot simulate input: the device is paused by the EIS implementation",
        )),
    }
}

/// Split `s` into UTF-8 substrings of at most `max_bytes` bytes each.
fn utf8_byte_chunks(mut s: &str, max_bytes: usize) -> impl Iterator<Item = &str> {
    // A chunk must be able to hold any UTF-8 char (up to 4 bytes), otherwise
    // no progress could be made and empty chunks would be yielded forever
    debug_assert!(max_bytes >= 4);
    std::iter::from_fn(move || {
        if s.is_empty() {
            return None;
        }
        let mut end = max_bytes.min(s.len());
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let (chunk, rest) = s.split_at(end);
        s = rest;
        Some(chunk)
    })
}
