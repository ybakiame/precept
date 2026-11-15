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
        // Retire either half of a wide glyph touched by a diff repaint. Cursor
        // jumps over blank cells must not retain old text or collapse columns.
        if self.x > 0 && self.rows[self.y][self.x].is_none() {
            self.rows[self.y][self.x - 1] = Some(' ');
        }
        let end = self.x + width;
        if end < self.rows[0].len() && self.rows[self.y][end].is_none() {
            self.rows[self.y][end] = Some(' ');
        }
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
        let mut params = p.iter();
        let n = params.next().map_or(0, |p| usize::from(p[0]));
        let one = n.max(1);
        match c {
            'n' if n == 6 => self
                .responses
                .push(format!("\x1b[{};{}R", self.y + 1, self.x + 1).into_bytes()),
            'H' | 'f' => {
                self.y = one - 1;
                self.x = params.next().map_or(1, |p| usize::from(p[0])).max(1) - 1
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
            'X' => {
                self.clamp();
                let row = &mut self.rows[self.y];
                let start = if self.x > 0 && row[self.x].is_none() {
                    self.x - 1
                } else {
                    self.x
                };
                let mut end = self.x.saturating_add(one).min(row.len());
                if end < row.len() && row[end].is_none() {
                    end += 1;
                }
                row[start..end].fill(Some(' '));
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
    #[test]
    fn narrow_diff_over_wide_glyph_keeps_the_skipped_space() {
        let mut s = Screen::new(20, 3);
        let mut p = vte::Parser::new();
        // A wide zh-CN glyph pair, then an en diff repaint that writes the words
        // with cursor jumps over the space cells (the backend skips them). The
        // retired continuation cells must read as spaces, not collapse the words.
        p.advance(&mut s, "写入授权".as_bytes());
        assert!(s.text().contains("写入授权"));
        p.advance(&mut s, b"\x1b[1;1HWrite\x1b[1;7Hgrant\x1b[1;13Hoff");
        assert!(s.text().contains("Write grant off"));
    }
    #[test]
    fn conpty_erasure_and_cursor_jumps_replace_old_draft_text() {
        let mut s = Screen::new(80, 3);
        let mut p = vte::Parser::new();
        p.advance(&mut s, b"\x1b[2;3Hsynthetic/old-draft-path");
        p.advance(
            &mut s,
            b"\x1b[2;3H\x1b[78XPrepared\x1b[1Coperation;\x1b[1Cno\x1b[1Cwrite\x1b[1Csent\x1b[1Cyet",
        );
        assert_eq!(
            s.text().lines().nth(1).unwrap().trim_end(),
            "  Prepared operation; no write sent yet"
        );
        p.advance(&mut s, b"\x1b[6n");
        assert_eq!(s.responses, vec![b"\x1b[2;40R".to_vec()]);
    }
    #[test]
    fn partial_wide_glyph_erasure_and_overlay_preserve_visible_columns() {
        let mut s = Screen::new(12, 3);
        let mut p = vte::Parser::new();
        p.advance(&mut s, "中文tail".as_bytes());
        p.advance(&mut s, b"\x1b[1;2H\x1b[0X|");
        assert_eq!(s.text().lines().next().unwrap().trim_end(), " |文tail");
        p.advance(&mut s, b"\x1b[1;4H!");
        assert_eq!(s.text().lines().next().unwrap().trim_end(), " | !tail");
        p.advance(&mut s, "\x1b[2;1H中文\x1b[2;2H新".as_bytes());
        assert_eq!(s.text().lines().nth(1).unwrap().trim_end(), " 新");
    }
}
