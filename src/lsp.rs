use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};

use serde_json::{json, Value};

use crate::compiler;

pub fn serve() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    let mut documents: HashMap<String, String> = HashMap::new();
    loop {
        // read headers
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {}
                Err(_) => return,
            }
            if line == "\r\n" || line == "\n" || line.is_empty() {
                break;
            }
            if let Some(rest) = line.strip_prefix("Content-Length:") {
                content_length = rest.trim().parse().ok();
            }
        }
        let Some(length) = content_length else {
            continue;
        };
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let text = String::from_utf8_lossy(&body);
        let request: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = request.get("id").cloned();
        match method {
            "initialize" => {
                let result = json!({
                    "capabilities": {
                        "textDocumentSync": 1,
                        "completionProvider": { "triggerCharacters": ["."] },
                        "definitionProvider": true,
                        "renameProvider": { "prepareProvider": true }
                    }
                });
                send_response(&mut writer, id, result);
            }
            "initialized" => {}
            "textDocument/didOpen" => {
                if let Some(params) = request.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string();
                    let text = params
                        .get("textDocument")
                        .and_then(|t| t.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();
                    documents.insert(uri.clone(), text.clone());
                    publish_diagnostics(&mut writer, &uri, &text);
                }
            }
            "textDocument/didChange" => {
                if let Some(params) = request.get("params") {
                    let uri = params
                        .get("textDocument")
                        .and_then(|t| t.get("uri"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string();
                    if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array()) {
                        if let Some(last) = changes.last() {
                            if let Some(text) = last.get("text").and_then(|t| t.as_str()) {
                                documents.insert(uri.clone(), text.to_string());
                                publish_diagnostics(&mut writer, &uri, text);
                            }
                        }
                    }
                }
            }
            "textDocument/definition" => {
                let result = definition_response(&documents, &request);
                send_response(&mut writer, id, result);
            }
            "textDocument/rename" => {
                let result = rename_response(&documents, &request);
                send_response(&mut writer, id, result);
            }
            "textDocument/prepareRename" => {
                let result = prepare_rename_response(&documents, &request);
                send_response(&mut writer, id, result);
            }
            "textDocument/completion" => {
                let items = json!([
                    { "label": "fn" }, { "label": "int" }, { "label": "float" },
                    { "label": "bool" }, { "label": "str" }, { "label": "class" },
                    { "label": "struct" }, { "label": "interface" }, { "label": "enum" },
                    { "label": "union" }, { "label": "match" }, { "label": "if" },
                    { "label": "while" }, { "label": "for" }, { "label": "return" },
                    { "label": "print" }, { "label": "printf" }, { "label": "new" }
                ]);
                send_response(&mut writer, id, items);
            }
            "shutdown" => {
                send_response(&mut writer, id, Value::Null);
            }
            "exit" => return,
            _ => {
                if id.is_some() {
                    send_response(&mut writer, id, Value::Null);
                }
            }
        }
    }
}


/// Converts a zero-based LSP line/character pair into a byte offset.
fn offset_at(text: &str, line: usize, character: usize) -> Option<usize> {
    let mut offset = 0;
    for (index, current) in text.split_inclusive('\n').enumerate() {
        if index == line {
            return Some(offset + character.min(current.len()));
        }
        offset += current.len();
    }
    if line == text.lines().count() {
        return Some(text.len());
    }
    None
}

/// Converts a byte offset into a zero-based LSP line/character pair.
fn position_at(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 0;
    let mut start = 0;
    for (index, byte) in text.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            start = index + 1;
        }
    }
    (line, offset - start)
}

fn request_position(request: &Value) -> Option<(usize, usize)> {
    let position = request.get("params")?.get("position")?;
    Some((
        position.get("line")?.as_u64()? as usize,
        position.get("character")?.as_u64()? as usize,
    ))
}

fn request_uri(request: &Value) -> Option<String> {
    request
        .get("params")?
        .get("textDocument")?
        .get("uri")?
        .as_str()
        .map(str::to_string)
}

fn word_at(text: &str, offset: usize) -> Option<(String, usize, usize)> {
    let bytes = text.as_bytes();
    let is_ident = |byte: u8| byte == b'_' || byte.is_ascii_alphanumeric();
    let mut start = offset.min(bytes.len());
    while start > 0 && is_ident(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(bytes.len());
    while end < bytes.len() && is_ident(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some((text[start..end].to_string(), start, end))
}

/// Finds the first declaration of `name` in a document.
fn find_definition(text: &str, name: &str) -> Option<(usize, usize)> {
    const DECLARATION_KEYWORDS: &[&str] = &[
        "fn", "class", "struct", "interface", "trait", "enum", "union", "mixin", "type",
        "int", "float", "str", "bool", "void", "auto", "list", "table", "dict", "const",
        "local", "static", "private", "protected", "public", "abstract", "final",
    ];
    let is_ident = |c: char| c == '_' || c.is_ascii_alphanumeric();
    for (line_index, line) in text.lines().enumerate() {
        let mut tokens: Vec<(usize, &str)> = Vec::new();
        let mut cursor = 0;
        let bytes = line.as_bytes();
        while cursor < bytes.len() {
            if is_ident(bytes[cursor] as char) {
                let start = cursor;
                while cursor < bytes.len() && is_ident(bytes[cursor] as char) {
                    cursor += 1;
                }
                tokens.push((start, &line[start..cursor]));
            } else {
                cursor += 1;
            }
        }
        for (index, (column, token)) in tokens.iter().enumerate() {
            if *token != name {
                continue;
            }
            let declared = index > 0
                && DECLARATION_KEYWORDS.contains(&tokens[index - 1].1)
                && !matches!(tokens[index.saturating_sub(2)].1, "return" | "new");
            if declared {
                return Some((line_index, *column));
            }
        }
    }
    None
}

fn definition_response(documents: &HashMap<String, String>, request: &Value) -> Value {
    let Some(uri) = request_uri(request) else {
        return Value::Null;
    };
    let Some((line, character)) = request_position(request) else {
        return Value::Null;
    };
    let Some(text) = documents.get(&uri) else {
        return Value::Null;
    };
    let Some(offset) = offset_at(text, line, character) else {
        return Value::Null;
    };
    let Some((name, _, _)) = word_at(text, offset) else {
        return Value::Null;
    };
    let Some((target_line, target_character)) = find_definition(text, &name) else {
        return Value::Null;
    };
    json!({
        "uri": uri,
        "range": {
            "start": { "line": target_line, "character": target_character },
            "end": { "line": target_line, "character": target_character + name.len() }
        }
    })
}

fn prepare_rename_response(documents: &HashMap<String, String>, request: &Value) -> Value {
    let Some(uri) = request_uri(request) else {
        return Value::Null;
    };
    let Some((line, character)) = request_position(request) else {
        return Value::Null;
    };
    let Some(text) = documents.get(&uri) else {
        return Value::Null;
    };
    let Some(offset) = offset_at(text, line, character) else {
        return Value::Null;
    };
    let Some((name, start, end)) = word_at(text, offset) else {
        return Value::Null;
    };
    let (start_line, start_character) = position_at(text, start);
    let (end_line, end_character) = position_at(text, end);
    json!({
        "range": {
            "start": { "line": start_line, "character": start_character },
            "end": { "line": end_line, "character": end_character }
        },
        "placeholder": name
    })
}

fn rename_response(documents: &HashMap<String, String>, request: &Value) -> Value {
    let Some(uri) = request_uri(request) else {
        return Value::Null;
    };
    let Some((line, character)) = request_position(request) else {
        return Value::Null;
    };
    let new_name = request
        .get("params")
        .and_then(|params| params.get("newName"))
        .and_then(|name| name.as_str())
        .unwrap_or("");
    let valid = !new_name.is_empty()
        && new_name
            .chars()
            .next()
            .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
        && new_name
            .chars()
            .all(|c| c == '_' || c.is_ascii_alphanumeric());
    if !valid {
        return Value::Null;
    }
    let Some(text) = documents.get(&uri) else {
        return Value::Null;
    };
    let Some(offset) = offset_at(text, line, character) else {
        return Value::Null;
    };
    let Some((name, _, _)) = word_at(text, offset) else {
        return Value::Null;
    };
    let mut edits = Vec::new();
    for (index, source_line) in text.lines().enumerate() {
        let bytes = source_line.as_bytes();
        let mut cursor = 0;
        while cursor < bytes.len() {
            if bytes[cursor] == b'_' || bytes[cursor].is_ascii_alphanumeric() {
                let start = cursor;
                while cursor < bytes.len()
                    && (bytes[cursor] == b'_' || bytes[cursor].is_ascii_alphanumeric())
                {
                    cursor += 1;
                }
                if &source_line[start..cursor] == name {
                    edits.push(json!({
                        "range": {
                            "start": { "line": index, "character": start },
                            "end": { "line": index, "character": cursor }
                        },
                        "newText": new_name
                    }));
                }
            } else {
                cursor += 1;
            }
        }
    }
    if edits.is_empty() {
        return Value::Null;
    }
    json!({ "changes": { uri: edits } })
}

fn publish_diagnostics(writer: &mut impl Write, uri: &str, text: &str) {
    let diagnostics = match compiler::compile(text) {
        Ok(_) => vec![],
        Err(errors) => errors
            .into_iter()
            .map(|d| {
                json!({
                    "range": {
                        "start": { "line": d.line.unwrap_or(1).saturating_sub(1), "character": d.column.unwrap_or(1).saturating_sub(1) },
                        "end": { "line": d.line.unwrap_or(1).saturating_sub(1), "character": d.column.unwrap_or(1).saturating_sub(1) }
                    },
                    "severity": 1,
                    "message": d.message
                })
            })
            .collect::<Vec<_>>(),
    };
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": diagnostics
        }
    });
    send_message(writer, &notification.to_string());
}

fn send_response(writer: &mut impl Write, id: Option<Value>, result: Value) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    send_message(writer, &response.to_string());
}

fn send_message(writer: &mut impl Write, body: &str) {
    let _ = write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = writer.flush();
}