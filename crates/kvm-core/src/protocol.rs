use serde::{Deserialize, Serialize};

/// Wire protocol version. Bump on breaking changes.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Message {
    Hello {
        name: String,
        screen_w: u32,
        screen_h: u32,
        version: u32,
    },
    Welcome {
        peer_id: u64,
    },
    MouseMove {
        dx: i32,
        dy: i32,
    },
    MouseButton {
        button: u8,
        down: bool,
    },
    MouseWheel {
        dx: i32,
        dy: i32,
    },
    KeyEvent {
        code: u32,
        down: bool,
    },
    EnterScreen {
        peer_id: u64,
        entry_x_ratio: f32,
        entry_y_ratio: f32,
    },
    LeaveScreen,
    Clipboard {
        format: String,
        data: Vec<u8>,
    },
    FileOffer {
        id: u64,
        name: String,
        size: u64,
    },
    FileChunk {
        id: u64,
        seq: u32,
        bytes: Vec<u8>,
    },
    FileEnd {
        id: u64,
    },
    Heartbeat,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(msg: &Message) -> Message {
        let bytes = bincode::serialize(msg).expect("serialize failed");
        bincode::deserialize(&bytes).expect("deserialize failed")
    }

    #[test]
    fn hello() {
        let m = Message::Hello {
            name: "TestPC".into(),
            screen_w: 1920,
            screen_h: 1080,
            version: VERSION,
        };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn welcome() {
        let m = Message::Welcome { peer_id: 42 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn mouse_move() {
        let m = Message::MouseMove { dx: -5, dy: 100 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn mouse_button() {
        let m = Message::MouseButton { button: 0, down: true };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn mouse_wheel() {
        let m = Message::MouseWheel { dx: 0, dy: -3 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn key_event() {
        let m = Message::KeyEvent { code: 65, down: false };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn enter_screen() {
        let m = Message::EnterScreen {
            peer_id: 7,
            entry_x_ratio: 0.0,
            entry_y_ratio: 0.5,
        };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn leave_screen() {
        let m = Message::LeaveScreen;
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn clipboard() {
        let m = Message::Clipboard {
            format: "text/plain".into(),
            data: b"hello clipboard".to_vec(),
        };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn file_offer() {
        let m = Message::FileOffer {
            id: 1,
            name: "photo.jpg".into(),
            size: 1_048_576,
        };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn file_chunk() {
        let m = Message::FileChunk {
            id: 1,
            seq: 3,
            bytes: vec![0u8; 512],
        };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn file_end() {
        let m = Message::FileEnd { id: 1 };
        assert_eq!(round_trip(&m), m);
    }

    #[test]
    fn heartbeat() {
        let m = Message::Heartbeat;
        assert_eq!(round_trip(&m), m);
    }
}
