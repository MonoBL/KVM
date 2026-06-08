use crate::keymap::rdev_to_code;
use crate::protocol::Message;
use rdev::{Event, EventType};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Convert a non-motion rdev Event to a protocol Message, if mappable.
///
/// Mouse motion is intentionally NOT handled here: rdev reports the
/// ABSOLUTE cursor position, but the wire protocol carries RELATIVE
/// deltas. Use [`Converter`] (or the server engine) to turn absolute
/// positions into deltas.
pub fn event_to_msg(event: &Event) -> Option<Message> {
    match &event.event_type {
        EventType::MouseMove { .. } => None,
        EventType::ButtonPress(btn) => Some(Message::MouseButton {
            button: button_id(btn),
            down: true,
        }),
        EventType::ButtonRelease(btn) => Some(Message::MouseButton {
            button: button_id(btn),
            down: false,
        }),
        EventType::Wheel { delta_x, delta_y } => Some(Message::MouseWheel {
            dx: *delta_x as i32,
            dy: *delta_y as i32,
        }),
        EventType::KeyPress(key) => rdev_to_code(key).map(|code| Message::KeyEvent {
            code,
            down: true,
        }),
        EventType::KeyRelease(key) => rdev_to_code(key).map(|code| Message::KeyEvent {
            code,
            down: false,
        }),
    }
}

fn button_id(btn: &rdev::Button) -> u8 {
    use rdev::Button::*;
    match btn {
        Left => 0,
        Right => 1,
        Middle => 2,
        Unknown(n) => *n,
    }
}

/// Stateful event converter that turns absolute mouse positions into
/// relative deltas (C1). One instance per capture stream.
pub struct Converter {
    last_mouse: Option<(f64, f64)>,
}

impl Default for Converter {
    fn default() -> Self {
        Self { last_mouse: None }
    }
}

impl Converter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert an event to a protocol Message. Mouse moves become deltas
    /// relative to the previous position; the first move seeds the
    /// reference and emits nothing.
    pub fn convert(&mut self, event: &Event) -> Option<Message> {
        if let EventType::MouseMove { x, y } = event.event_type {
            let (lx, ly) = self.last_mouse.unwrap_or((x, y));
            self.last_mouse = Some((x, y));
            let dx = (x - lx).round() as i32;
            let dy = (y - ly).round() as i32;
            if dx == 0 && dy == 0 {
                return None;
            }
            return Some(Message::MouseMove { dx, dy });
        }
        event_to_msg(event)
    }
}

/// Start a non-blocking rdev listen loop (no suppression).
/// Used for testing and for listen-only mode.
/// Sends converted messages to `tx`; stops forwarding once `tx` is closed.
pub fn start_listen(tx: Sender<Message>) {
    std::thread::spawn(move || {
        let mut conv = Converter::new();
        rdev::listen(move |event| {
            if let Some(msg) = conv.convert(&event) {
                // If the receiver is gone, stop forwarding. We do NOT exit
                // the process: rdev::listen has no cancel, so the thread is
                // left idle rather than killing the whole app (H2).
                let _ = tx.send(msg);
            }
        })
        .ok();
    });
}

/// Event delivered to the server grab callback.
#[derive(Debug)]
pub enum ServerGrabEvent {
    /// Absolute cursor position reported by the OS. The engine converts to
    /// deltas internally via `on_mouse_abs`.
    MouseAbs(i32, i32),
    /// Any other input event already encoded as a protocol message.
    Msg(Message),
}

/// Server-side grab loop.
///
/// Delivers absolute mouse positions as `ServerGrabEvent::MouseAbs` and all
/// other events (key, button, wheel) as `ServerGrabEvent::Msg`.  The callback
/// returns `true` to suppress the event from the local system.
///
/// Threading notes (macOS):
///   rdev::grab calls CFRunLoopRun() on the calling thread, creating a run
///   loop for it. On macOS 10.11+ CGEvent taps work from non-main threads.
///   Spawn this in a dedicated `std::thread` (not the Tauri main thread).
///   Requires Accessibility + Input Monitoring permissions at runtime.
///   PENDING: manual verification on macOS with permissions granted.
pub fn run_server_grab_loop(on_event: impl FnMut(ServerGrabEvent) -> bool + Send + 'static) {
    let on_event = Mutex::new(on_event);
    rdev::grab(move |event| {
        let sge = match &event.event_type {
            EventType::MouseMove { x, y } => ServerGrabEvent::MouseAbs(*x as i32, *y as i32),
            _ => match event_to_msg(&event) {
                Some(m) => ServerGrabEvent::Msg(m),
                None => return Some(event), // unmapped key/button -> pass through
            },
        };
        if on_event.lock().unwrap()(sge) {
            None // suppressed
        } else {
            Some(event)
        }
    })
    .ok();
}

/// Start a grab loop that calls `on_msg(msg)` for every captured event and
/// suppresses the event from the local system when it returns `true`.
///
/// Mouse moves are delivered as relative deltas via an internal [`Converter`].
///
/// Threading notes (macOS): same as `run_server_grab_loop`.
/// Requires Accessibility + Input Monitoring permissions.
pub fn run_grab_loop(on_msg: impl FnMut(Message) -> bool + Send + 'static) {
    // rdev::grab requires Fn, so wrap the FnMut state in Mutexes.
    let on_msg = Mutex::new(on_msg);
    let conv = Mutex::new(Converter::new());
    rdev::grab(move |event| {
        let msg = conv.lock().unwrap().convert(&event);
        if let Some(m) = msg {
            let suppress = on_msg.lock().unwrap()(m);
            if suppress {
                return None;
            }
        }
        Some(event)
    })
    .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdev::{Button, Key};

    fn ev(t: EventType) -> Event {
        Event {
            time: std::time::SystemTime::UNIX_EPOCH,
            name: None,
            event_type: t,
        }
    }

    #[test]
    fn button_ids() {
        assert_eq!(button_id(&Button::Left), 0);
        assert_eq!(button_id(&Button::Right), 1);
        assert_eq!(button_id(&Button::Middle), 2);
    }

    #[test]
    fn event_to_msg_key_press() {
        let msg = event_to_msg(&ev(EventType::KeyPress(Key::KeyA))).unwrap();
        assert!(matches!(msg, Message::KeyEvent { code: 0x04, down: true }));
    }

    #[test]
    fn event_to_msg_mouse_button() {
        let msg = event_to_msg(&ev(EventType::ButtonPress(Button::Left))).unwrap();
        assert!(matches!(msg, Message::MouseButton { button: 0, down: true }));
    }

    #[test]
    fn event_to_msg_wheel() {
        let msg = event_to_msg(&ev(EventType::Wheel { delta_x: 0, delta_y: -3 })).unwrap();
        assert!(matches!(msg, Message::MouseWheel { dx: 0, dy: -3 }));
    }

    #[test]
    fn raw_mouse_move_is_not_a_delta() {
        // Absolute position must not be emitted directly as a delta.
        assert!(event_to_msg(&ev(EventType::MouseMove { x: 500.0, y: 300.0 })).is_none());
    }

    /// C1 regression: converter emits true deltas, not absolute coords.
    #[test]
    fn converter_produces_deltas() {
        let mut c = Converter::new();
        // First move seeds the reference, emits nothing.
        assert!(c.convert(&ev(EventType::MouseMove { x: 500.0, y: 300.0 })).is_none());
        // Second move: delta from the first.
        let msg = c.convert(&ev(EventType::MouseMove { x: 510.0, y: 295.0 })).unwrap();
        assert_eq!(msg, Message::MouseMove { dx: 10, dy: -5 });
        // No movement emits nothing.
        assert!(c.convert(&ev(EventType::MouseMove { x: 510.0, y: 295.0 })).is_none());
    }

    #[test]
    fn server_grab_event_mouse_abs() {
        // MouseMove should become ServerGrabEvent::MouseAbs, not a delta.
        // We can't call run_server_grab_loop (needs grab perms) but we can
        // verify the routing logic used inside it.
        let e = ev(EventType::MouseMove { x: 1234.0, y: 567.0 });
        assert!(event_to_msg(&e).is_none()); // event_to_msg ignores moves
        // The run_server_grab_loop closure would produce MouseAbs(1234, 567).
    }

    #[test]
    fn server_grab_event_key() {
        let e = ev(EventType::KeyPress(Key::KeyA));
        let msg = event_to_msg(&e).unwrap();
        assert!(matches!(msg, Message::KeyEvent { code: 0x04, down: true }));
        // run_server_grab_loop would wrap this as ServerGrabEvent::Msg(msg).
    }
}
