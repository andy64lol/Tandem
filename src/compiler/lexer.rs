#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Ident(String),
    Num(String),
    Str(String),
    Sym(String),
    Eof,
}
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub(crate) kind: Tok,
    pub(crate) pos: usize,
}

fn long_bracket_open(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut cursor = start + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'[') {
        Some((cursor + 1, cursor - start - 1))
    } else {
        None
    }
}

fn skip_long_bracket_comment(
    bytes: &[u8],
    start: usize,
    body_start: usize,
    level: usize,
) -> Result<usize, String> {
    let mut cursor = body_start;
    while cursor < bytes.len() {
        if bytes[cursor] == b']' {
            let mut close = cursor + 1;
            let mut close_level = 0;
            while bytes.get(close) == Some(&b'=') {
                close += 1;
                close_level += 1;
            }
            if close_level == level && bytes.get(close) == Some(&b']') {
                return Ok(close + 1);
            }
        }
        cursor += 1;
    }
    Err(format!("unterminated Lua long comment at byte {start}"))
}

pub(crate) fn lex(src: &str) -> Result<Vec<Token>, String> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = vec![];
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 >= b.len() {
                return Err(format!("unterminated block comment at byte {start}"));
            }
            i += 2;
            continue;
        }
        if b[i] == b'-' && i + 1 < b.len() && b[i + 1] == b'-' {
            let start = i;
            i += 2;
            if let Some((body_start, level)) = long_bracket_open(b, i) {
                i = skip_long_bracket_comment(b, start, body_start, level)?;
            } else {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            continue;
        }
        let start = i;
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Token {
                kind: Tok::Ident(src[start..i].into()),
                pos: start,
            });
            continue;
        }
        if b[i].is_ascii_digit() {
            i += 1;
            while i < b.len()
                && (b[i].is_ascii_digit()
                    || (b[i] == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()))
            {
                i += 1;
            }
            out.push(Token {
                kind: Tok::Num(src[start..i].into()),
                pos: start,
            });
            continue;
        }
        if b[i] == b'"' {
            i += 1;
            let mut val = String::new();
            let mut closed = false;
            while i < b.len() {
                let c = b[i];
                i += 1;
                if c == b'"' {
                    closed = true;
                    break;
                }
                if c == b'\\' {
                    if i >= b.len() {
                        break;
                    }
                    let e = b[i];
                    i += 1;
                    val.push(match e {
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'"' => '"',
                        b'\\' => '\\',
                        _ => return Err(format!("unknown string escape at byte {}", i - 2)),
                    });
                } else {
                    val.push(c as char);
                }
            }
            if !closed {
                return Err(format!("unterminated string at byte {start}"));
            }
            out.push(Token {
                kind: Tok::Str(val),
                pos: start,
            });
            continue;
        }
        let rest = &src[i..];
        let op = [
            "==", "!=", "<=", ">=", "->", "&&", "||", "+=", "-=", "*=", "/=",
        ]
        .into_iter()
        .find(|op| rest.starts_with(op));
        if let Some(op) = op {
            out.push(Token {
                kind: Tok::Sym(op.into()),
                pos: i,
            });
            i += op.len();
            continue;
        }
        let c = b[i] as char;
        if "{}(),;+-*/%=<>!".contains(c) {
            out.push(Token {
                kind: Tok::Sym(c.to_string()),
                pos: i,
            });
            i += 1;
            continue;
        }
        return Err(format!("unexpected character {:?} at byte {}", c, i));
    }
    out.push(Token {
        kind: Tok::Eof,
        pos: src.len(),
    });
    Ok(out)
}
