use std::path::Path;

use mlua::Lua;
use mlua_pkg::{ops, resolvers::VendoredResolver, Config, PkgDir, Project, Registry};

const TANDEM_MANIFEST: &str = "tandem.config";
const TANDEM_LOCK: &str = "tandem.lock";
const LEGACY_MANIFEST: &str = "mlua-pkg.toml";
const LEGACY_LOCK: &str = "mlua-pkg.lock";

/// Resolve the manifest/lock filenames for a project root, preferring
/// Tandem's own Cargo-like names and falling back to the legacy ones.
fn manifest_names(root: &Path) -> (String, String) {
    if root.join(TANDEM_MANIFEST).is_file() {
        (TANDEM_MANIFEST.to_string(), TANDEM_LOCK.to_string())
    } else if root.join(LEGACY_MANIFEST).is_file() {
        (LEGACY_MANIFEST.to_string(), LEGACY_LOCK.to_string())
    } else {
        // Default to Tandem's own names for new projects.
        (TANDEM_MANIFEST.to_string(), TANDEM_LOCK.to_string())
    }
}

/// Install the dependencies declared in `tandem.config` (or `mlua-pkg.toml`)
/// under `root`.
pub fn install(root: impl AsRef<Path>) -> Result<(), String> {
    let root = root.as_ref();
    let (manifest, lock) = manifest_names(root);
    let project = Project::new(
        root.join(&manifest),
        root.join(&lock),
        PkgDir::default_in(root),
    );
    let config = Config::new(project);
    ops::install(&config)
        .map(|_| ())
        .map_err(|error| format!("cannot install Lua packages: {error}"))
}

pub(crate) fn install_resolver(lua: &Lua, root: &Path) -> Result<(), String> {
    let lock = if root.join(TANDEM_LOCK).is_file() {
        TANDEM_LOCK
    } else {
        LEGACY_LOCK
    };
    let lockfile = root.join(lock);
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

/// Adds a git-backed dependency to `tandem.config` and installs it.
///
/// A bare package name is resolved against the registry base URL from
/// `TANDEM_REGISTRY` (default `https://github.com`), matching the
/// `https://<base>/<name>/<name>.git` convention.
pub fn add(
    root: impl AsRef<Path>,
    name: &str,
    git: Option<&str>,
    tag: Option<&str>,
) -> Result<(), String> {
    let root = root.as_ref();
    let (manifest, lock) = manifest_names(root);
    let project = Project::new(
        root.join(&manifest),
        root.join(&lock),
        PkgDir::default_in(root),
    );
    let config = Config::new(project);
    let registry = std::env::var("TANDEM_REGISTRY")
        .unwrap_or_else(|_| "https://github.com".to_string());
    let derived = git
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}/{name}/{name}.git", registry.trim_end_matches('/')));
    let mut spec = ops::AddSpec::new(name, derived);
    spec.tag = tag.map(str::to_string);
    ops::add(&config, spec)
        .map_err(|error| format!("cannot add package '{name}': {error}"))?;
    ops::install(&config)
        .map(|_| ())
        .map_err(|error| format!("cannot install package '{name}': {error}"))
}

/// The manifest filename Tandem prefers.
pub fn manifest_name() -> &'static str {
    TANDEM_MANIFEST
}

/// The lockfile filename Tandem prefers.
pub fn lock_name() -> &'static str {
    TANDEM_LOCK
}