use super::report::{Diagnostic, Severity, message};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Position {
    pub(super) line: u32,
    pub(super) column: u32,
    pub(super) end_line: u32,
    pub(super) end_column: u32,
}

impl Position {
    pub(super) fn start() -> Self {
        Self {
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
        }
    }
}

pub(super) struct Positions<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> Positions<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Self { text, line_starts }
    }

    pub(super) fn span(&self, range: Range<usize>) -> Position {
        let start = self.point(range.start);
        let end = self.point(range.end.max(range.start));
        Position {
            line: start.0,
            column: start.1,
            end_line: end.0,
            end_column: end.1,
        }
    }

    fn point(&self, offset: usize) -> (u32, u32) {
        let offset = offset.min(self.text.len());
        let line = self
            .line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1);
        let start = self.line_starts[line];
        let column = self.text[start..offset].chars().count() + 1;
        (number(line + 1), number(column))
    }
}

fn number(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub(super) fn diagnostic(
    code: &'static str,
    severity: Severity,
    position: Position,
    related_file: Option<String>,
) -> Diagnostic {
    Diagnostic {
        code: code.to_owned(),
        severity,
        message: message(code).to_owned(),
        line: position.line,
        column: position.column,
        end_line: position.end_line,
        end_column: position.end_column,
        related_file,
    }
}
