use crate::protocol::Message;
use anyhow::Result;
use arboard::Clipboard;

pub const FORMAT_TEXT: &str = "text/plain";

/// Read current clipboard text and return a Clipboard message.
pub fn make_clipboard_msg() -> Result<Option<Message>> {
    let mut cb = Clipboard::new()?;
    match cb.get_text() {
        Ok(text) if !text.is_empty() => Ok(Some(Message::Clipboard {
            format: FORMAT_TEXT.into(),
            data: text.into_bytes(),
        })),
        _ => Ok(None),
    }
}

/// Apply a received Clipboard message to the local clipboard.
pub fn apply_clipboard_msg(msg: &Message) -> Result<()> {
    if let Message::Clipboard { format, data } = msg {
        if format == FORMAT_TEXT {
            let text = String::from_utf8_lossy(data).into_owned();
            let mut cb = Clipboard::new()?;
            cb.set_text(text)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_message_round_trip() {
        let msg = Message::Clipboard {
            format: FORMAT_TEXT.into(),
            data: b"hopper clipboard test".to_vec(),
        };
        let bytes = bincode::serialize(&msg).unwrap();
        let decoded: Message = bincode::deserialize(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }

    // Live read/write test requires a display. Run manually:
    // DISPLAY=:0 cargo test -p kvm-core clipboard_live -- --ignored
    #[test]
    #[ignore]
    fn clipboard_live() {
        let mut cb = Clipboard::new().unwrap();
        cb.set_text("hopper-test-string").unwrap();
        let got = cb.get_text().unwrap();
        assert_eq!(got, "hopper-test-string");
    }
}
