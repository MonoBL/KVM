use crate::layout::{crossed_edge, entry_ratio, neighbor, Edge, Screen};
use crate::protocol::Message;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    Server,
    Client,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Local,
    Remote { peer_id: u64 },
}

/// What the server must do to the LOCAL machine after a mouse sample.
#[derive(Debug, Default, PartialEq)]
pub struct MouseOutcome {
    /// Messages to send to the active remote peer, in order.
    pub messages: Vec<Message>,
    /// Warp the LOCAL physical cursor to this position (local-screen px).
    /// Used to park the cursor while remote, and to restore it on return.
    pub warp: Option<(i32, i32)>,
    /// Show (false) or hide (true) the LOCAL cursor, if it changed.
    pub set_hidden: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub role: Role,
    pub control: Control,
    pub screens: Vec<Screen>,
    /// Current virtual cursor position on the LOCAL screen (server only).
    pub cursor_x: i32,
    pub cursor_y: i32,
    /// ID of the local screen (server: index 0 by convention).
    pub local_screen_id: u64,
    // ---- remote-control bookkeeping (server, while control is Remote) ----
    /// The screen we handed control to.
    remote_screen: Option<Screen>,
    /// Virtual cursor on the remote screen, in remote-screen px.
    remote_cursor: (f32, f32),
    /// The LOCAL edge that was crossed to enter the remote screen.
    entry_edge: Option<Edge>,
    /// Local point the physical cursor is parked at while remote.
    park: (i32, i32),
}

impl EngineState {
    pub fn new_server(local_screen: Screen) -> Self {
        Self {
            role: Role::Server,
            control: Control::Local,
            screens: vec![local_screen],
            cursor_x: 0,
            cursor_y: 0,
            local_screen_id: 0,
            remote_screen: None,
            remote_cursor: (0.0, 0.0),
            entry_edge: None,
            park: (0, 0),
        }
    }

    pub fn new_client() -> Self {
        Self {
            role: Role::Client,
            control: Control::Local,
            screens: vec![],
            cursor_x: 0,
            cursor_y: 0,
            local_screen_id: 0,
            remote_screen: None,
            remote_cursor: (0.0, 0.0),
            entry_edge: None,
            park: (0, 0),
        }
    }

    pub fn add_screen(&mut self, screen: Screen) {
        self.screens.push(screen);
    }

    fn local_screen(&self) -> Option<&Screen> {
        self.screens.iter().find(|s| s.id == self.local_screen_id)
    }

    /// Forward a key/button/wheel message to the remote peer, but only while
    /// control is Remote. Returns None when control is Local so that local
    /// input is never leaked to clients.
    pub fn forward(&self, msg: Message) -> Option<Message> {
        match self.control {
            Control::Remote { .. } => Some(msg),
            Control::Local => None,
        }
    }

    /// Server-side: feed one ABSOLUTE mouse position from the grab loop.
    /// Returns what to send to the peer and how to move/park the local cursor.
    ///
    /// Live-wiring contract: while control is Remote the caller must apply
    /// `warp` (so the physical cursor stays parked and deltas stay relative)
    /// and `set_hidden` after each call.
    pub fn on_mouse_abs(&mut self, x: i32, y: i32) -> MouseOutcome {
        match self.control {
            Control::Local => self.on_mouse_local(x, y),
            Control::Remote { peer_id } => self.on_mouse_remote(x, y, peer_id),
        }
    }

    fn on_mouse_local(&mut self, x: i32, y: i32) -> MouseOutcome {
        self.cursor_x = x;
        self.cursor_y = y;
        let mut out = MouseOutcome::default();

        let local = match self.local_screen() {
            Some(s) => s.clone(),
            None => return out,
        };
        let Some(edge) = crossed_edge(x, y, &local) else {
            return out;
        };
        let Some(nb) = neighbor(&self.screens, &local, &edge) else {
            tracing::warn!(
                "edge {:?} hit at ({},{}) on local {}x{} but NO neighbor; screens(id,col,row)={:?}",
                edge, x, y, local.width, local.height,
                self.screens.iter().map(|s| (s.id, s.col, s.row)).collect::<Vec<_>>()
            );
            return out; // edge of the world: stay local
        };
        let nb = nb.clone();
        tracing::info!("crossing {:?} -> peer {} ({}x{})", edge, nb.id, nb.width, nb.height);

        // Hand control to the neighbor.
        let (ex, ey) = entry_ratio(x, y, &local, &edge);
        self.control = Control::Remote { peer_id: nb.id };
        self.remote_cursor = (ex * nb.width as f32, ey * nb.height as f32);
        self.remote_screen = Some(nb.clone());
        self.entry_edge = Some(edge);
        self.park = (local.width as i32 / 2, local.height as i32 / 2);

        out.messages.push(Message::EnterScreen {
            peer_id: nb.id,
            entry_x_ratio: ex,
            entry_y_ratio: ey,
        });
        out.warp = Some(self.park);
        out.set_hidden = Some(true);
        out
    }

    fn on_mouse_remote(&mut self, x: i32, y: i32, _peer_id: u64) -> MouseOutcome {
        let mut out = MouseOutcome::default();
        let Some(remote) = self.remote_screen.clone() else {
            return out;
        };

        // Delta measured from the park point; cursor is re-parked each event.
        let dx = x - self.park.0;
        let dy = y - self.park.1;
        out.warp = Some(self.park);

        let rw = remote.width as f32;
        let rh = remote.height as f32;
        self.remote_cursor.0 += dx as f32;
        self.remote_cursor.1 += dy as f32;

        // Did the virtual cursor cross back over the entry edge?
        if self.crossed_back(rw, rh) {
            return self.return_to_local(&remote);
        }

        // Stay remote: clamp and forward the delta.
        self.remote_cursor.0 = self.remote_cursor.0.clamp(0.0, rw - 1.0);
        self.remote_cursor.1 = self.remote_cursor.1.clamp(0.0, rh - 1.0);
        if dx != 0 || dy != 0 {
            out.messages.push(Message::MouseMove { dx, dy });
        }
        out
    }

    fn crossed_back(&self, rw: f32, rh: f32) -> bool {
        let (rx, ry) = self.remote_cursor;
        // Entry via a given LOCAL edge means we entered the OPPOSITE side of
        // the remote; returning means crossing back out of that opposite side.
        match self.entry_edge {
            Some(Edge::Right) => rx < 0.0,   // entered remote-left, leave left
            Some(Edge::Left) => rx >= rw,    // entered remote-right, leave right
            Some(Edge::Bottom) => ry < 0.0,  // entered remote-top, leave top
            Some(Edge::Top) => ry >= rh,     // entered remote-bottom, leave bottom
            None => false,
        }
    }

    fn return_to_local(&mut self, remote: &Screen) -> MouseOutcome {
        let local = self.local_screen().cloned();
        let mut out = MouseOutcome::default();

        // Reappear on the local screen at the same edge we left from.
        if let Some(local) = local {
            let lw = local.width as i32;
            let lh = local.height as i32;
            let ry = (self.remote_cursor.1 / remote.height as f32).clamp(0.0, 1.0);
            let rx = (self.remote_cursor.0 / remote.width as f32).clamp(0.0, 1.0);
            let pos = match self.entry_edge {
                Some(Edge::Right) => (lw - 1, (ry * lh as f32) as i32),
                Some(Edge::Left) => (0, (ry * lh as f32) as i32),
                Some(Edge::Bottom) => ((rx * lw as f32) as i32, lh - 1),
                Some(Edge::Top) => ((rx * lw as f32) as i32, 0),
                None => (lw / 2, lh / 2),
            };
            out.warp = Some(pos);
            self.cursor_x = pos.0;
            self.cursor_y = pos.1;
        }

        self.control = Control::Local;
        self.remote_screen = None;
        self.entry_edge = None;
        out.messages.push(Message::LeaveScreen);
        out.set_hidden = Some(false);
        out
    }

    /// Server-side: process a LeaveScreen sent by a remote peer (fallback
    /// path if the client detects the return edge itself).
    pub fn on_leave_screen(&mut self) -> Option<Message> {
        if matches!(self.control, Control::Remote { .. }) {
            self.control = Control::Local;
            self.remote_screen = None;
            self.entry_edge = None;
            Some(Message::LeaveScreen)
        } else {
            None
        }
    }

    /// Unregister a peer that disconnected. Removes its screen and, if
    /// control was remote to that peer, resets to Local.
    pub fn remove_peer(&mut self, peer_id: u64) {
        self.screens.retain(|s| s.id != peer_id);
        if matches!(self.control, Control::Remote { peer_id: pid } if pid == peer_id) {
            self.control = Control::Local;
            self.remote_screen = None;
            self.entry_edge = None;
        }
    }
}

pub type SharedState = Arc<Mutex<EngineState>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_screen(id: u64, col: i32, row: i32) -> Screen {
        Screen { id, name: format!("S{}", id), width: 1920, height: 1080, col, row }
    }

    fn server_two() -> EngineState {
        let mut s = EngineState::new_server(make_screen(0, 0, 0));
        s.add_screen(make_screen(1, 1, 0)); // neighbor to the right
        s
    }

    #[test]
    fn local_stays_local_when_inside() {
        let mut s = server_two();
        let out = s.on_mouse_abs(960, 540);
        assert!(out.messages.is_empty());
        assert_eq!(s.control, Control::Local);
    }

    #[test]
    fn crossing_right_switches_to_remote() {
        let mut s = server_two();
        let out = s.on_mouse_abs(1920, 540);
        assert!(matches!(s.control, Control::Remote { peer_id: 1 }));
        assert_eq!(out.set_hidden, Some(true));
        assert!(out.warp.is_some()); // parked
        match &out.messages[0] {
            Message::EnterScreen { peer_id, entry_x_ratio, .. } => {
                assert_eq!(*peer_id, 1);
                assert_eq!(*entry_x_ratio, 0.0);
            }
            _ => panic!("expected EnterScreen"),
        }
    }

    #[test]
    fn no_neighbor_no_switch() {
        let mut s = EngineState::new_server(make_screen(0, 0, 0));
        let out = s.on_mouse_abs(1920, 540);
        assert!(out.messages.is_empty());
        assert_eq!(s.control, Control::Local);
    }

    #[test]
    fn remote_moves_forward_as_deltas() {
        let mut s = server_two();
        s.on_mouse_abs(1920, 540); // switch; park = (960, 540)
        // Physical mouse nudges right of park -> positive dx forwarded.
        let out = s.on_mouse_abs(970, 545);
        assert!(matches!(s.control, Control::Remote { .. }));
        assert_eq!(out.warp, Some((960, 540))); // re-parked
        assert_eq!(out.messages, vec![Message::MouseMove { dx: 10, dy: 5 }]);
    }

    /// C2 regression: moving the virtual cursor back over the entry edge
    /// returns control to local and restores the cursor.
    #[test]
    fn crossing_back_returns_to_local() {
        let mut s = server_two();
        s.on_mouse_abs(1920, 540); // entered remote from the left edge
        // remote_cursor starts at x=0; push far left of park to drive it < 0.
        let out = s.on_mouse_abs(960 - 50, 540);
        assert_eq!(s.control, Control::Local);
        assert_eq!(out.set_hidden, Some(false));
        assert!(out.messages.contains(&Message::LeaveScreen));
        assert!(out.warp.is_some()); // cursor restored on local screen
    }

    #[test]
    fn local_input_not_forwarded() {
        let s = server_two(); // control Local
        assert!(s.forward(Message::KeyEvent { code: 0x04, down: true }).is_none());
    }

    #[test]
    fn remote_input_is_forwarded() {
        let mut s = server_two();
        s.on_mouse_abs(1920, 540); // now Remote
        assert!(s.forward(Message::KeyEvent { code: 0x04, down: true }).is_some());
    }

    #[test]
    fn leave_screen_message_restores_local() {
        let mut s = server_two();
        s.on_mouse_abs(1920, 540);
        assert_eq!(s.on_leave_screen(), Some(Message::LeaveScreen));
        assert_eq!(s.control, Control::Local);
    }

    #[test]
    fn remove_peer_unregisters_screen_and_resets_control() {
        let mut s = server_two(); // screens: [S0, S1]; S1.id=1
        // Go remote on S1
        s.on_mouse_abs(1920, 540);
        assert!(matches!(s.control, Control::Remote { peer_id: 1 }));
        assert_eq!(s.screens.len(), 2);

        // Disconnect peer 1 (e.g. TCP closed)
        s.remove_peer(1);
        assert_eq!(s.control, Control::Local, "should return to local");
        assert_eq!(s.screens.len(), 1, "peer screen removed");
        assert_eq!(s.screens[0].id, 0, "local screen remains");
    }

    #[test]
    fn remove_peer_noop_when_not_remote() {
        let mut s = server_two();
        // No crossing - still Local
        s.remove_peer(1);
        assert_eq!(s.control, Control::Local);
        assert_eq!(s.screens.len(), 1);
    }
}
