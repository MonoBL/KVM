/// Virtual screen grid and edge-crossing math.

#[derive(Debug, Clone, PartialEq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub id: u64,
    /// Name shown in the GUI.
    pub name: String,
    /// Pixel dimensions of this screen.
    pub width: u32,
    pub height: u32,
    /// Grid position (column, row). Origin is top-left.
    pub col: i32,
    pub row: i32,
}

/// Check whether (x, y) has crossed the given edge of a screen.
pub fn crossed_edge(x: i32, y: i32, screen: &Screen) -> Option<Edge> {
    let w = screen.width as i32;
    let h = screen.height as i32;
    if x < 0 {
        return Some(Edge::Left);
    }
    if x >= w {
        return Some(Edge::Right);
    }
    if y < 0 {
        return Some(Edge::Top);
    }
    if y >= h {
        return Some(Edge::Bottom);
    }
    None
}

/// Find the neighbor screen in the given direction, if any.
pub fn neighbor<'a>(screens: &'a [Screen], current: &Screen, edge: &Edge) -> Option<&'a Screen> {
    let (dc, dr) = match edge {
        Edge::Right => (1, 0),
        Edge::Left => (-1, 0),
        Edge::Bottom => (0, 1),
        Edge::Top => (0, -1),
    };
    let target_col = current.col + dc;
    let target_row = current.row + dr;
    screens.iter().find(|s| s.col == target_col && s.row == target_row)
}

/// Compute the entry position ratio on the neighbor screen when crossing an edge.
/// ratio is in 0..1 representing where along the perpendicular axis we crossed.
pub fn entry_ratio(x: i32, y: i32, screen: &Screen, edge: &Edge) -> (f32, f32) {
    let w = screen.width as f32;
    let h = screen.height as f32;
    match edge {
        Edge::Left | Edge::Right => {
            // crossing horizontal border: y is the relevant axis
            let entry_x = match edge {
                Edge::Left => 1.0,  // enter from right side of neighbor
                Edge::Right => 0.0, // enter from left side of neighbor
                _ => unreachable!(),
            };
            let entry_y = (y as f32).clamp(0.0, h - 1.0) / h;
            (entry_x, entry_y)
        }
        Edge::Top | Edge::Bottom => {
            let entry_x = (x as f32).clamp(0.0, w - 1.0) / w;
            let entry_y = match edge {
                Edge::Top => 1.0,    // enter from bottom of neighbor
                Edge::Bottom => 0.0, // enter from top of neighbor
                _ => unreachable!(),
            };
            (entry_x, entry_y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_screen(id: u64, w: u32, h: u32, col: i32, row: i32) -> Screen {
        Screen { id, name: format!("S{}", id), width: w, height: h, col, row }
    }

    #[test]
    fn no_edge_inside_screen() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        assert_eq!(crossed_edge(960, 540, &s), None);
        assert_eq!(crossed_edge(0, 0, &s), None);
        assert_eq!(crossed_edge(1919, 1079, &s), None);
    }

    #[test]
    fn right_edge() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        assert_eq!(crossed_edge(1920, 540, &s), Some(Edge::Right));
    }

    #[test]
    fn left_edge() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        assert_eq!(crossed_edge(-1, 540, &s), Some(Edge::Left));
    }

    #[test]
    fn top_edge() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        assert_eq!(crossed_edge(960, -1, &s), Some(Edge::Top));
    }

    #[test]
    fn bottom_edge() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        assert_eq!(crossed_edge(960, 1080, &s), Some(Edge::Bottom));
    }

    #[test]
    fn neighbor_right() {
        let screens = vec![
            make_screen(0, 1920, 1080, 0, 0),
            make_screen(1, 1920, 1080, 1, 0),
        ];
        let n = neighbor(&screens, &screens[0], &Edge::Right).unwrap();
        assert_eq!(n.id, 1);
    }

    #[test]
    fn neighbor_missing() {
        let screens = vec![make_screen(0, 1920, 1080, 0, 0)];
        assert!(neighbor(&screens, &screens[0], &Edge::Right).is_none());
    }

    #[test]
    fn entry_ratio_right_edge_midpoint() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        let (ex, ey) = entry_ratio(1920, 540, &s, &Edge::Right);
        assert_eq!(ex, 0.0);
        assert!((ey - 0.5).abs() < 0.01, "ey={}", ey);
    }

    #[test]
    fn entry_ratio_left_edge() {
        let s = make_screen(0, 1920, 1080, 1, 0);
        let (ex, _ey) = entry_ratio(-1, 540, &s, &Edge::Left);
        assert_eq!(ex, 1.0);
    }

    #[test]
    fn entry_ratio_bottom_edge_midpoint() {
        let s = make_screen(0, 1920, 1080, 0, 0);
        let (ex, ey) = entry_ratio(960, 1080, &s, &Edge::Bottom);
        assert!((ex - 0.5).abs() < 0.01, "ex={}", ex);
        assert_eq!(ey, 0.0);
    }

    #[test]
    fn two_screen_horizontal_layout() {
        let screens = vec![
            make_screen(0, 1920, 1080, 0, 0),
            make_screen(1, 2560, 1440, 1, 0),
        ];
        // Cursor at right edge of screen 0
        let edge = crossed_edge(1920, 540, &screens[0]).unwrap();
        assert_eq!(edge, Edge::Right);
        let nb = neighbor(&screens, &screens[0], &edge).unwrap();
        assert_eq!(nb.id, 1);
        let (ex, ey) = entry_ratio(1920, 540, &screens[0], &edge);
        // enters at left side of screen 1, mid-height
        assert_eq!(ex, 0.0);
        assert!((ey - 0.5).abs() < 0.01);
    }
}
