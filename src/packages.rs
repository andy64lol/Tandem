use std::path::Path;

use mlua::Lua;
use mlua_pkg::{ops, resolvers::VendoredResolver, Config, PkgDir, Project, Registry};

/// Install the dependencies declared in `mlua-pkg.toml` under `root`.
pub fn install(root: impl AsRef<Path>) -> Result<(), String> {
    let root = root.as_ref();
    let project = Project::in_dir(root, PkgDir::default_in(root));
    let config = Config::new(project);
    ops::install(&config)
        .map(|_| ())
        .map_err(|error| format!("cannot install Lua packages: {error}"))
}

pub(crate) fn install_resolver(lua: &Lua, root: &Path) -> Result<(), String> {
    let lockfile = root.join("mlua-pkg.lock");
    if !lockfile.is_file() {
        return Ok(());
    }

    let package_dir = PkgDir::default_in(root);
    let resolver = VendoredResolver::from_lockfile(&lockfile, package_dir.vendored())
        .map_err(|error| format!("cannot load Lua package lockfile: {error}"))?;
    let mut registry = Registry::new();
    registry.add(resolver);
    registry
        .install(lua)
        .map_err(|error| format!("cannot configure Lua package resolver: {error}"))
}
