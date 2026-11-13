//! A small Debug Adapter Protocol server for Tandem.
//!
//! The adapter speaks the standard `Content-Length`-framed JSON protocol on
//! stdin/stdout.  It supports launching a `.tdm` program, line breakpoints
//! mapped back to Tandem source, continuing, pausing, and streaming program
//! output as DAP `output` events.  The debuggee runs on a background thread so
//! the adapter can keep answering requests while it is paused.

use std::{
    collections::HashSet,
    fs,
    io::{self, BufRead, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
};

use serde_json::{json, Value};

use crate::{compiler, runtime};

/// A JSON-RPC style channel shared by the request loop and the debuggee.
#[derive(Clone)]
struct Wire {
    writer: Arc<Mutex<BufWriter<io::Stdout>>>,
    seq: Arc<AtomicI64>,
}

impl Wire {
    fn new() -> Self {
        Self {
            writer: Arc::new(Mutex::new(BufWriter::new(io::stdout()))),
            seq: Arc::new(AtomicI64::new(1)),
        }
    }

    fn send(&self, mut message: Value) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        if let Some(object) = message.as_object_mut() {
            object.insert("seq".into(), json!(seq));
        }
        let Ok(body) = serde_json::to_vec(&message) else {
            return;
        };
        let mut writer = self.writer.lock().unwrap();
        let _ = write!(writer, "Content-Length: {}\r\n\r\n", body.len());
        let _ = writer.write_all(&body);
        let _ = writer.flush();
    }

    fn respond(&self, request: &Value, body: Value) {
        self.send(json!({
            "type": "response",
            "request_seq": request.get("seq").and_then(Value::as_i64).unwrap_or(0),
            "success": true,
            "command": request.get("command").and_then(Value::as_str).unwrap_or(""),
            "body": body,
        }));
    }

    fn event(&self, name: &str, body: Value) {
        self.send(json!({ "type": "event", "event": name, "body": body }));
    }
}

#[derive(Default)]
struct DebugState {
    breakpoints: HashSet<usize>,
    paused: bool,
    pending_pause: bool,
    stopped_line: Option<usize>,
    source: Option<PathBuf>,
    finished: bool,
}

type Shared = Arc<(Mutex<DebugState>, Condvar)>;

/// Runs the adapter until the client disconnects.
pub fn serve() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let wire = Wire::new();
    let shared: Shared = Arc::new((Mutex::new(DebugState::default()), Condvar::new()));
    let mut launched = false;

    loop {
        let Ok(Some(request)) = read_message(&mut reader) else {
            break;
        };
        let command = request
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match command.as_str() {
            "initialize" => {
                wire.respond(
                    &request,
                    json!({
                        "supportsConfigurationDoneRequest": true,
                        "supportsTerminateRequest": true,
                        "supportsEvaluateForHovers": false,
                    }),
                );
                wire.event("initialized", json!({}));
            }
            "setBreakpoints" => {
                let source = request
                    .get("arguments")
                    .and_then(|arguments| arguments.get("source"))
                    .and_then(|source| source.get("path"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                let lines = request
                    .get("arguments")
                    .and_then(|arguments| arguments.get("breakpoints"))
                    .and_then(Value::as_array)
                    .map(|breakpoints| {
                        breakpoints
                            .iter()
                            .filter_map(|breakpoint| {
                                breakpoint.get("line").and_then(Value::as_u64)
                            })
                            .map(|line| line as usize)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                {
                    let (lock, _) = &*shared;
                    let mut state = lock.lock().unwrap();
                    state.breakpoints = lines.iter().copied().collect();
                    if state.source.is_none() {
                        state.source = source;
                    }
                }
                let body: Vec<Value> = lines
                    .iter()
                    .map(|line| json!({ "verified": true, "line": line }))
                    .collect();
                wire.respond(&request, json!({ "breakpoints": body }));
            }
            "launch" => {
                let path = request
                    .get("arguments")
                    .and_then(|arguments| arguments.get("program").or_else(|| arguments.get("source")))
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                {
                    let (lock, _) = &*shared;
                    let mut state = lock.lock().unwrap();
                    if state.source.is_none() {
                        state.source = path.clone();
                    }
                }
                wire.respond(&request, json!({}));
                if !launched {
                    launched = true;
                    if let Some(path) = path.or_else(|| {
                        let (lock, _) = &*shared;
                        lock.lock().unwrap().source.clone()
                    }) {
                        spawn_debuggee(wire.clone(), Arc::clone(&shared), path);
                    } else {
                        wire.event(
                            "output",
                            json!({ "category": "stderr", "output": "no program to launch\n" }),
                        );
                        wire.event("terminated", json!({}));
                    }
                }
            }
            "configurationDone" => wire.respond(&request, json!({})),
            "threads" => {
                wire.respond(
                    &request,
                    json!({ "threads": [{ "id": 1, "name": "main" }] }),
                );
            }
            "stackTrace" => {
                let source = {
                    let (lock, _) = &*shared;
                    lock.lock().unwrap().source.clone()
                };
                let line = {
                    let (lock, _) = &*shared;
                    lock.lock().unwrap().stopped_line
                };
                let frames = match (source, line) {
                    (Some(path), Some(line)) => json!([{
                        "id": 1,
                        "name": "main",
                        "source": { "name": path.file_name().and_then(|n| n.to_str()).unwrap_or("main.tdm"), "path": path },
                        "line": line,
                        "column": 1,
                    }]),
                    _ => json!([]),
                };
                wire.respond(&request, json!({ "stackFrames": frames }));
            }
            "scopes" | "variables" => wire.respond(&request, json!({ "scopes": [], "variables": [] })),
            "continue" | "next" | "stepIn" | "stepOut" => {
                resume(&shared);
                wire.respond(&request, json!({ "allThreadsContinued": true }));
            }
            "pause" => {
                let (lock, _) = &*shared;
                lock.lock().unwrap().pending_pause = true;
                wire.respond(&request, json!({}));
            }
            "disconnect" | "terminate" => {
                resume(&shared);
                wire.respond(&request, json!({}));
                break;
            }
            "evaluate" => {
                wire.respond(
                    &request,
                    json!({ "result": "", "variablesReference": 0 }),
                );
            }
            _ => wire.respond(&request, json!({})),
        }
    }
}

fn resume(shared: &Shared) {
    let (lock, condvar) = &**shared;
    let mut state = lock.lock().unwrap();
    state.paused = false;
    state.pending_pause = false;
    state.stopped_line = None;
    condvar.notify_all();
}

fn spawn_debuggee(wire: Wire, shared: Shared, path: PathBuf) {
    thread::spawn(move || {
        {
            let (lock, _) = &*shared;
            lock.lock().unwrap().source = Some(path.clone());
        }
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) => {
                report(&wire, &format!("cannot read {}: {error}", path.display()));
                finish(&wire, &shared);
                return;
            }
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "main.tdm".to_string());
        let compiled = match compiler::compile_with_dir_named(&source, &parent, &name) {
            Ok(compiled) => compiled,
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    report(&wire, &diagnostic.message);
                }
                finish(&wire, &shared);
                return;
            }
        };

        let mapping = Arc::new(runtime::line_map(&compiled));
        let hook_state = Arc::clone(&shared);
        let hook_wire = wire.clone();
        let hook_mapping = Arc::clone(&mapping);
        let print_wire = wire.clone();
        let lua_name = path.to_string_lossy().into_owned();
        let result = runtime::run_source_with_setup(
            &compiled,
            &lua_name,
            &[],
            &parent,
            &runtime::RuntimeConfig::default(),
            move |lua| {
                let print = lua
                    .create_function(move |_, args: mlua::MultiValue| {
                        let parts = args
                            .iter()
                            .map(|value| value.to_string().unwrap_or_default())
                            .collect::<Vec<_>>();
                        print_wire.event(
                            "output",
                            json!({
                                "category": "stdout",
                                "output": format!("{}\n", parts.join("\t")),
                            }),
                        );
                        Ok(())
                    })
                    .map_err(|error| error.to_string())?;
                lua.globals()
                    .set("print", print)
                    .map_err(|error| error.to_string())?;
                lua.set_hook(
                    mlua::HookTriggers::new().every_line(),
                    move |_, debug| {
                        let Some(line) = debug.current_line() else {
                            return Ok(mlua::VmState::Continue);
                        };
                        let Some(Some(tdm)) = hook_mapping.get(line.saturating_sub(1)) else {
                            return Ok(mlua::VmState::Continue);
                        };
                        let (lock, condvar) = &*hook_state;
                        let mut state = lock.lock().unwrap();
                        if state.finished {
                            return Ok(mlua::VmState::Continue);
                        }
                        if state.breakpoints.contains(tdm) || state.pending_pause {
                            state.pending_pause = false;
                            state.paused = true;
                            state.stopped_line = Some(*tdm);
                            hook_wire.event(
                                "stopped",
                                json!({
                                    "reason": "breakpoint",
                                    "threadId": 1,
                                    "allThreadsStopped": true,
                                }),
                            );
                            while state.paused {
                                state = condvar.wait(state).unwrap();
                            }
                        }
                        Ok(mlua::VmState::Continue)
                    },
                )
                .map_err(|error| format!("cannot set breakpoint hook: {error}"))
            },
        );
        match result {
            Ok(()) => {}
            Err(error) => report(&wire, &error),
        }
        finish(&wire, &shared);
    });
}

fn finish(wire: &Wire, shared: &Shared) {
    {
        let (lock, _) = &**shared;
        let mut state = lock.lock().unwrap();
        state.finished = true;
        state.paused = false;
        state.stopped_line = None;
    }
    wire.event("terminated", json!({}));
}

fn report(wire: &Wire, message: &str) {
    wire.event(
        "output",
        json!({ "category": "stderr", "output": format!("{message}\n") }),
    );
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = content_length else {
        return Ok(None);
    };
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body).ok())
}
