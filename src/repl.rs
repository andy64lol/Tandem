use std::io::{self, BufRead, Write};

use crate::compiler;
use crate::runtime;

pub fn run() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut buffer = String::new();
    let mut line = String::new();
    print!("tandem> ");
    let _ = io::stdout().flush();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) => {
                eprintln!("read error: {error}");
                break;
            }
        }
        let trimmed = line.trim();
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }
        if trimmed.is_empty() {
            if !buffer.trim().is_empty() {
                evaluate(&buffer);
                buffer.clear();
            }
            print!("tandem> ");
            let _ = io::stdout().flush();
            continue;
        }
        buffer.push_str(&line);
        print!("... ");
        let _ = io::stdout().flush();
    }
    if !buffer.trim().is_empty() {
        evaluate(&buffer);
    }
}

fn evaluate(source: &str) {
    match compiler::compile(source) {
        Ok(lua) => {
            if let Err(error) = runtime::run_source(&lua, "repl.tdm") {
                eprintln!("runtime error: {error}");
            }
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}", diagnostic.message);
            }
        }
    }
}