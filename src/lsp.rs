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
                        "completionProvider": { "triggerCharacters": ["."] }
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