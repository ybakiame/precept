use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Root,
    AppliesTo,
    Check(usize),
    Guard(usize),
    Other,
}

struct Field {
    scope: Scope,
    name: String,
    span: Range<usize>,
}

struct Table {
    scope: Scope,
    span: Range<usize>,
}

pub(super) struct Locations {
    fields: Vec<Field>,
    tables: Vec<Table>,
}

impl Locations {
    pub(super) fn scan(front: &str) -> Self {
        let mut locations = Self {
            fields: vec![],
            tables: vec![],
        };
        let mut cursor = 0;
        let mut scope = Scope::Root;
        let mut checks = 0;
        let mut guards = 0;
        while cursor < front.len() {
            cursor = skip_trivia(front, cursor);
            if cursor >= front.len() {
                break;
            }
            if front.as_bytes()[cursor] == b'[' {
                let (header_scope, end) = scan_header(front, cursor, checks, guards);
                if let Some(header_scope) = header_scope {
                    locations.tables.push(Table {
                        scope: header_scope,
                        span: cursor..end,
                    });
                }
                scope = header_scope.unwrap_or(Scope::Other);
                if matches!(scope, Scope::Check(_)) {
                    checks += 1;
                }
                if matches!(scope, Scope::Guard(_)) {
                    guards += 1;
                }
                cursor = end;
                continue;
            }
            let Some(equals) = find_unquoted(front, cursor, b'=') else {
                break;
            };
            let key_end = front[cursor..equals].trim_end().len() + cursor;
            let path = key_path(front[cursor..key_end].trim());
            let value_start = skip_horizontal(front, equals + 1);
            let end = value_end(front, value_start);
            let (field_scope, name) = match path.as_slice() {
                [name] => (scope, name.clone()),
                [first, second] if first == "applies_to" => (Scope::AppliesTo, second.clone()),
                _ => (Scope::Other, path.join(".")),
            };
            if !matches!(field_scope, Scope::Other) {
                locations.fields.push(Field {
                    scope: field_scope,
                    name,
                    span: cursor..key_end,
                });
            }
            cursor = end;
        }
        locations
    }

    pub(super) fn field(&self, scope: Scope, name: &str) -> Option<Range<usize>> {
        self.fields
            .iter()
            .find(|field| field.scope == scope && field.name == name)
            .map(|field| field.span.clone())
    }

    pub(super) fn table(&self, scope: Scope) -> Option<Range<usize>> {
        self.tables
            .iter()
            .find(|table| table.scope == scope)
            .map(|table| table.span.clone())
    }

    pub(super) fn last_table(&self, scope: Scope) -> Option<Range<usize>> {
        self.tables
            .iter()
            .rev()
            .find(|table| table.scope == scope)
            .map(|table| table.span.clone())
    }
}

fn scan_header(front: &str, start: usize, checks: usize, guards: usize) -> (Option<Scope>, usize) {
    let bytes = front.as_bytes();
    let array = bytes.get(start + 1) == Some(&b'[');
    let inner_start = start + if array { 2 } else { 1 };
    let mut cursor = inner_start;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active) = quote {
            if active == b'"' && byte == b'\\' && !escaped {
                escaped = true;
            } else {
                if byte == active && !escaped {
                    quote = None;
                }
                escaped = false;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if byte == b']' {
            let end = if array && bytes.get(cursor + 1) == Some(&b']') {
                cursor + 2
            } else {
                cursor + 1
            };
            let path = key_path(front[inner_start..cursor].trim());
            let header_scope = match path.as_slice() {
                [name] if name == "applies_to" && !array => Some(Scope::AppliesTo),
                [name] if name == "check" && array => Some(Scope::Check(checks)),
                [name] if name == "guard" && array => Some(Scope::Guard(guards)),
                _ => None,
            };
            return (header_scope, end);
        } else if byte == b'\n' {
            break;
        }
        cursor += 1;
    }
    (None, cursor)
}

fn key_path(key: &str) -> Vec<String> {
    let mut parts = vec![];
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;
    for (index, byte) in key.bytes().enumerate() {
        if let Some(active) = quote {
            if active == b'"' && byte == b'\\' && !escaped {
                escaped = true;
            } else {
                if byte == active && !escaped {
                    quote = None;
                }
                escaped = false;
            }
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if byte == b'.' {
            parts.push(decode_key(key[start..index].trim()));
            start = index + 1;
        }
    }
    parts.push(decode_key(key[start..].trim()));
    parts
}

fn decode_key(key: &str) -> String {
    if key.starts_with('"') && key.ends_with('"') {
        toml::from_str::<String>(key).unwrap_or_else(|_| key.to_owned())
    } else if key.starts_with('\'') && key.ends_with('\'') {
        key[1..key.len() - 1].to_owned()
    } else {
        key.to_owned()
    }
}

fn find_unquoted(text: &str, start: usize, wanted: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut quote = None;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate().skip(start) {
        if let Some(active) = quote {
            if active == b'"' && *byte == b'\\' && !escaped {
                escaped = true;
            } else {
                if *byte == active && !escaped {
                    quote = None;
                }
                escaped = false;
            }
        } else if matches!(*byte, b'\'' | b'"') {
            quote = Some(*byte);
        } else if *byte == wanted {
            return Some(index);
        }
    }
    None
}

fn skip_trivia(text: &str, mut cursor: usize) -> usize {
    let bytes = text.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        } else if bytes[cursor] == b'#' {
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
        } else {
            break;
        }
    }
    cursor
}

fn skip_horizontal(text: &str, mut cursor: usize) -> usize {
    while text
        .as_bytes()
        .get(cursor)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        cursor += 1;
    }
    cursor
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    Basic,
    Literal,
    BasicMulti,
    LiteralMulti,
}

fn value_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut cursor = start;
    let mut square = 0usize;
    let mut curly = 0usize;
    let mut quote = None;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        match quote {
            Some(Quote::Basic) => {
                if byte == b'\\' {
                    cursor = (cursor + 1).min(bytes.len());
                } else if byte == b'"' {
                    quote = None;
                }
            }
            Some(Quote::Literal) => {
                if byte == b'\'' {
                    quote = None;
                }
            }
            Some(Quote::BasicMulti) => {
                if byte == b'\\' {
                    cursor = (cursor + 1).min(bytes.len());
                } else if bytes.get(cursor..cursor + 3) == Some(b"\"\"\"") {
                    let mut run = 3;
                    while bytes.get(cursor + run) == Some(&b'"') {
                        run += 1;
                    }
                    quote = None;
                    cursor += run - 1;
                }
            }
            Some(Quote::LiteralMulti) => {
                if bytes.get(cursor..cursor + 3) == Some(b"'''") {
                    let mut run = 3;
                    while bytes.get(cursor + run) == Some(&b'\'') {
                        run += 1;
                    }
                    quote = None;
                    cursor += run - 1;
                }
            }
            None => match byte {
                b'"' => {
                    quote = if bytes.get(cursor..cursor + 3) == Some(b"\"\"\"") {
                        cursor += 2;
                        Some(Quote::BasicMulti)
                    } else {
                        Some(Quote::Basic)
                    };
                }
                b'\'' => {
                    quote = if bytes.get(cursor..cursor + 3) == Some(b"'''") {
                        cursor += 2;
                        Some(Quote::LiteralMulti)
                    } else {
                        Some(Quote::Literal)
                    };
                }
                b'[' => square += 1,
                b']' => square = square.saturating_sub(1),
                b'{' => curly += 1,
                b'}' => curly = curly.saturating_sub(1),
                b'#' => {
                    while cursor < bytes.len() && bytes[cursor] != b'\n' {
                        cursor += 1;
                    }
                    continue;
                }
                b'\n' if square == 0 && curly == 0 => return cursor,
                _ => {}
            },
        }
        cursor += 1;
    }
    bytes.len()
}
