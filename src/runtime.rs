use std::{fs, path::Path};

use mlua::Lua;

pub fn run_file(path: impl AsRef<Path>) -> Result<(), String> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_args(path: impl AsRef<Path>, args: &[String]) -> Result<(), String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read Lua file {}: {error}", path.display()))?;
    let name = path.to_string_lossy();
    run_source_with_args(&source, &name, args)
}

pub fn run_source(source: &str, name: &str) -> Result<(), String> {
    run_source_with_args(source, name, &[])
}

fn run_source_with_args(source: &str, name: &str, args: &[String]) -> Result<(), String> {
    let lua = Lua::new();
    let lua_args = lua
        .create_table()
        .map_err(|error| format!("cannot prepare Lua arguments: {error}"))?;
    lua_args
        .set(0, name)
        .map_err(|error| format!("cannot prepare Lua arguments: {error}"))?;
    for (index, arg) in args.iter().enumerate() {
        lua_args
            .set(index + 1, arg.as_str())
            .map_err(|error| format!("cannot prepare Lua arguments: {error}"))?;
    }
    lua.globals()
        .set("arg", lua_args)
        .map_err(|error| format!("cannot prepare Lua arguments: {error}"))?;
    lua.load(source)
        .set_name(name)
        .exec()
        .map_err(|error| format!("Lua runtime error in {name}: {error}"))
}
