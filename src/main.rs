use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

use typelua::{compiler, runtime};

fn usage() -> ! {
    eprintln!(
        "Usage: typelua <input.tlua|script.lua> [output.lua or script arguments...]\n       typelua --check <input.tlua>"
    );
    process::exit(2)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    if args[1] == "--check" {
        if args.len() != 3 {
            usage();
        }
        let source = fs::read_to_string(&args[2]).unwrap_or_else(|error| {
            eprintln!("cannot read {}: {error}", args[2]);
            process::exit(1)
        });
        compiler::compile(&source).unwrap_or_else(|error| {
            eprintln!("TypeLua error: {error}");
            process::exit(1)
        });
        println!("{}: OK", args[2]);
        return;
    }

    let input_path = Path::new(&args[1]);
    if input_path
        .extension()
        .is_some_and(|extension| extension == "lua")
    {
        runtime::run_file_with_args(input_path, &args[2..]).unwrap_or_else(|error| {
            eprintln!("{error}");
            process::exit(1)
        });
        return;
    }

    if args.len() > 3 {
        usage();
    }
    let source = fs::read_to_string(input_path).unwrap_or_else(|error| {
        eprintln!("cannot read {}: {error}", input_path.display());
        process::exit(1)
    });
    let lua_source = compiler::compile(&source).unwrap_or_else(|error| {
        eprintln!("TypeLua error: {error}");
        process::exit(1)
    });

    let output_path = args.get(2).map(PathBuf::from).unwrap_or_else(|| {
        let mut path = input_path.to_path_buf();
        path.set_extension("lua");
        path
    });
    fs::write(&output_path, lua_source).unwrap_or_else(|error| {
        eprintln!("cannot write {}: {error}", output_path.display());
        process::exit(1)
    });
    println!(
        "compiled {} -> {}",
        input_path.display(),
        output_path.display()
    );
    runtime::run_file(&output_path).unwrap_or_else(|error| {
        eprintln!("{error}");
        process::exit(1)
    });
}
