use std::{fs, path::Path};

use mlua::Lua;

pub fn run_file(path: impl AsRef<Path>) -> Result<(), String> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_args(path: impl AsRef<Path>, args: &[String]) -> Result<(), String> {
    let path = path.as_ref();
    let package_root = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    run_file_with_args_in(path, args, package_root)
}

pub fn run_file_with_args_in(
    path: impl AsRef<Path>,
    args: &[String],
    package_root: impl AsRef<Path>,
) -> Result<(), String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read Lua file {}: {error}", path.display()))?;
    let name = path.to_string_lossy();
    run_source_with_args(&source, &name, args, package_root.as_ref())
}

pub fn run_source(source: &str, name: &str) -> Result<(), String> {
    let path = Path::new(name);
    let package_root = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    run_source_with_args(source, name, &[], package_root)
}

fn run_source_with_args(
    source: &str,
    name: &str,
    args: &[String],
    package_root: &Path,
) -> Result<(), String> {
    let lua = Lua::new();
    crate::packages::install_resolver(&lua, package_root)?;
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
