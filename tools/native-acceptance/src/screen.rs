//! Minimal VT observation for this fixed NSH acceptance flow, not a new terminal.
use unicode_width::UnicodeWidthChar;
use vte::{Params, Perform};
pub struct Screen {
    pub rows: Vec<Vec<Option<char>>>,
    x: usize,
    y: usize,
    pub responses: Vec<Vec<u8>>,
}
impl Screen {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            rows: vec![vec![Some(' '); w]; h],
            x: 0,
            y: 0,
            responses: Vec::new(),
        }
    }
    pub fn text(&self) -> String {
        self.rows
            .iter()
            .map(|r| r.iter().flatten().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn resize(&mut self, w: usize, h: usize) {
        *self = Self::new(w, h);
    }
    fn clamp(&mut self) {
        self.y = self.y.min(self.rows.len() - 1);
        self.x = self.x.min(self.rows[0].len() - 1);
    }
}
impl Perform for Screen {
    fn print(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        if width == 0 {
            return;
        }
        self.clamp();
        self.rows[self.y][self.x] = Some(c);
        if width == 2 && self.x + 1 < self.rows[0].len() {
            self.rows[self.y][self.x + 1] = None;
        }
        self.x += width;
    }
    fn execute(&mut self, b: u8) {
        match b {
            b'\r' => self.x = 0,
            b'\n' => self.y += 1,
            8 => self.x = self.x.saturating_sub(1),
            _ => {}
        }
        self.clamp();
    }
    fn csi_dispatch(&mut self, p: &Params, _: &[u8], _: bool, c: char) {
        let a: Vec<_> = p.iter().map(|x| x[0] as usize).collect();
        let n = a.first().copied().unwrap_or(0);
        let one = n.max(1);
        match c {
            'n' if n == 6 => self
                .responses
                .push(format!("\x1b[{};{}R", self.y + 1, self.x + 1).into_bytes()),
            'H' | 'f' => {
                self.y = one - 1;
                self.x = a.get(1).copied().unwrap_or(1).max(1) - 1
            }
            'A' => self.y = self.y.saturating_sub(one),
            'B' => self.y += one,
            'C' => self.x += one,
            'D' => self.x = self.x.saturating_sub(one),
            'G' => self.x = one - 1,
            'd' => self.y = one - 1,
            'J' if n == 2 || n == 3 => {
                for r in &mut self.rows {
                    r.fill(Some(' '));
                }
            }
            'K' => {
                self.clamp();
                let row = &mut self.rows[self.y];
                match n {
                    0 => row[self.x..].fill(Some(' ')),
                    1 => row[..=self.x].fill(Some(' ')),
                    2 => row.fill(Some(' ')),
                    _ => {}
                }
            }
            _ => {}
        }
        self.clamp();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_cursor_redraw_and_wide_cells() {
        let mut s = Screen::new(20, 4);
        let mut p = vte::Parser::new();
        p.advance(&mut s, b"\x1b[2;3Hhello\x1b[2K\x1b[2;1H");
        p.advance(&mut s, "合成 Profile".as_bytes());
        assert!(s.text().contains("合成 Profile"));
        assert!(!s.text().contains("hello"));
        p.advance(&mut s, b"\x1b[3;7H\x1b[6n");
        assert_eq!(s.responses, vec![b"\x1b[3;7R".to_vec()]);
    }
}
