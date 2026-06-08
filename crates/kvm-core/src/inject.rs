use crate::keymap::code_to_enigo;
use crate::protocol::Message;
use anyhow::Result;
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};

/// Inject a received protocol message as local input.
/// Must be called from a thread with a display connection.
/// On macOS, requires Accessibility permission.
pub struct Injector {
    enigo: Enigo,
}

impl Injector {
    pub fn new() -> Result<Self> {
        let enigo = Enigo::new(&Settings::default())?;
        Ok(Self { enigo })
    }

    pub fn replay(&mut self, msg: &Message) -> Result<()> {
        match msg {
            Message::MouseMove { dx, dy } => {
                self.enigo.move_mouse(*dx, *dy, Coordinate::Rel)?;
            }
            Message::MouseButton { button, down } => {
                let btn = match button {
                    0 => Button::Left,
                    1 => Button::Right,
                    2 => Button::Middle,
                    _ => Button::Left,
                };
                let dir = if *down { Direction::Press } else { Direction::Release };
                self.enigo.button(btn, dir)?;
            }
            Message::MouseWheel { dx, dy } => {
                if *dy != 0 {
                    self.enigo.scroll(*dy, Axis::Vertical)?;
                }
                if *dx != 0 {
                    self.enigo.scroll(*dx, Axis::Horizontal)?;
                }
            }
            Message::KeyEvent { code, down } => {
                // Skip codes with no safe mapping on this platform rather
                // than inject a wrong key (see keymap::code_to_enigo).
                if let Some(key) = code_to_enigo(*code) {
                    let dir = if *down { Direction::Press } else { Direction::Release };
                    self.enigo.key(key, dir)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
