use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use mlua::{HookTriggers, Lua, VmState};

#[derive(Clone, Debug, Default)]
pub struct RuntimeConfig {
    pub memory_limit: Option<usize>,
    pub instruction_limit: Option<u64>,
}

pub fn run_file(path: impl AsRef<Path>) -> Result<(), String> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_args(path: impl AsRef<Path>, args: &[String]) -> Result<(), String> {
    let path = path.as_ref();
    let package_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    run_file_with_args_in(path, args, package_root)
}

pub fn run_file_with_args_in(
    path: impl AsRef<Path>,
    args: &[String],
    package_root: impl AsRef<Path>,
) -> Result<(), String> {
    run_file_with_config_in(path, args, package_root, &RuntimeConfig::default())
}

pub fn run_file_with_config(
    path: impl AsRef<Path>,
    args: &[String],
    config: &RuntimeConfig,
) -> Result<(), String> {
    let path = path.as_ref();
    let package_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    run_file_with_config_in(path, args, package_root, config)
}

pub fn run_file_with_config_in(
    path: impl AsRef<Path>,
    args: &[String],
    package_root: impl AsRef<Path>,
    config: &RuntimeConfig,
) -> Result<(), String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("cannot read Lua file {}: {error}", path.display()))?;
    let name = path.to_string_lossy();
    run_source_with_config(&source, &name, args, package_root.as_ref(), config)
}

pub fn run_source(source: &str, name: &str) -> Result<(), String> {
    let path = Path::new(name);
    let package_root = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    run_source_with_args(source, name, &[], package_root)
}

pub fn run_source_with_args(
    source: &str,
    name: &str,
    args: &[String],
    package_root: &Path,
) -> Result<(), String> {
    run_source_with_config(source, name, args, package_root, &RuntimeConfig::default())
}

pub fn run_source_with_config(
    source: &str,
    name: &str,
    args: &[String],
    package_root: &Path,
    config: &RuntimeConfig,
) -> Result<(), String> {
    run_source_with_setup(source, name, args, package_root, config, |_| Ok(()))
}

pub fn run_source_with_setup<F>(
    source: &str,
    name: &str,
    args: &[String],
    package_root: &Path,
    config: &RuntimeConfig,
    setup: F,
) -> Result<(), String>
where
    F: FnOnce(&Lua) -> Result<(), String>,
{
    let lua = Lua::new();
    if let Some(limit) = config.memory_limit {
        lua.set_memory_limit(limit)
            .map_err(|error| format!("cannot set Lua memory limit: {error}"))?;
    }
    if let Some(limit) = config.instruction_limit {
        let step = limit.clamp(1, 10_000) as u32;
        let executed = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&executed);
        lua.set_hook(
            HookTriggers::default().every_nth_instruction(step),
            move |_, _| {
                let count = counter.fetch_add(u64::from(step), Ordering::Relaxed) + u64::from(step);
                if count > limit {
                    Err(mlua::Error::RuntimeError(format!(
                        "instruction limit of {limit} exceeded"
                    )))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )
        .map_err(|error| format!("cannot set Lua instruction limit: {error}"))?;
    }
    configure_package_path(&lua, package_root)?;
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
    setup(&lua)?;
    lua.load(source)
        .set_name(name)
        .exec()
        .map_err(|error| format!("Lua runtime error in {name}: {error}"))
}

fn configure_package_path(lua: &Lua, package_root: &Path) -> Result<(), String> {
    let root = fs::canonicalize(package_root).unwrap_or_else(|_| package_root.to_path_buf());
    let root_string = root.to_string_lossy().replace('\\', "/");
    let package: mlua::Table = lua
        .globals()
        .get("package")
        .map_err(|error| format!("cannot configure Lua package path: {error}"))?;
    let current: String = package
        .get("path")
        .map_err(|error| format!("cannot configure Lua package path: {error}"))?;
    package
        .set(
            "path",
            format!("{root_string}/?.lua;{root_string}/?/init.lua;{current}"),
        )
        .map_err(|error| format!("cannot configure Lua package path: {error}"))?;

    let search_root = root.clone();
    let searcher = lua
        .create_function(move |lua, name: String| {
            if !name.ends_with(".lua") {
                return Ok(mlua::Value::Nil);
            }
            let path = search_root.join(&name);
            let source = match fs::read_to_string(&path) {
                Ok(source) => source,
                Err(_) => {
                    let message = format!("\n\tno file '{}'", path.display());
                    return Ok(mlua::Value::String(lua.create_string(message)?));
                }
            };
            let function = lua
                .load(&source)
                .set_name(path.to_string_lossy())
                .into_function()?;
            Ok(mlua::Value::Function(function))
        })
        .map_err(|error| format!("cannot configure Lua package search: {error}"))?;
    let searchers: mlua::Table = package
        .get("searchers")
        .map_err(|error| format!("cannot configure Lua package search: {error}"))?;
    searchers
        .raw_insert(2, searcher)
        .map_err(|error| format!("cannot configure Lua package search: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> std::path::PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tandem-runtime-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run_require(source: &str, dir: &Path) {
        let name = dir.join("main.lua");
        run_source_with_args(source, &name.to_string_lossy(), &[], dir).unwrap();
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn requires_lua_files_with_explicit_extensions() {
        let dir = temp_dir();
        fs::write(dir.join("helper.lua"), "return { value = 42 }").unwrap();
        run_require(
            "local helper = require(\"helper.lua\"); assert(helper.value == 42);",
            &dir,
        );
    }

    #[test]
    fn enforces_instruction_limits() {
        let config = RuntimeConfig {
            instruction_limit: Some(10_000),
            ..RuntimeConfig::default()
        };
        let error = run_source_with_config(
            "while true do end",
            "loop.lua",
            &[],
            Path::new("."),
            &config,
        )
        .unwrap_err();
        assert!(
            error.contains("instruction limit of 10000 exceeded"),
            "{error}"
        );
    }

    #[test]
    fn enforces_memory_limits() {
        let config = RuntimeConfig {
            memory_limit: Some(128 * 1024),
            ..RuntimeConfig::default()
        };
        let error = run_source_with_config(
            "local chunks = {} for i = 1, 1000 do chunks[i] = string.rep('x', 1024) end",
            "memory.lua",
            &[],
            Path::new("."),
            &config,
        )
        .unwrap_err();
        assert!(error.to_lowercase().contains("memory"), "{error}");
    }

    #[test]
    fn host_setup_can_install_lua_values() {
        run_source_with_setup(
            "assert(host_value == 42)",
            "host.lua",
            &[],
            Path::new("."),
            &RuntimeConfig::default(),
            |lua| {
                lua.globals()
                    .set("host_value", 42)
                    .map_err(|error| error.to_string())
            },
        )
        .unwrap();
    }

    #[test]
    fn requires_lua_files_without_extensions() {
        let dir = temp_dir();
        fs::write(dir.join("helper.lua"), "return { value = 42 }").unwrap();
        run_require(
            "local helper = require(\"helper\"); assert(helper.value == 42);",
            &dir,
        );
    }
}
