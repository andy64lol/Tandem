//! Cross-compilation targets for the Lua emitted by Tandem.
//!
//! Tandem targets Lua 5.4 by default, but the generated code only relies on a
//! small set of version-specific features.  [`LuaTarget`] rewrites the handful
//! of constructs that differ between Lua 5.1 and 5.4 so the same `.tdm` source
//! can be shipped to older interpreters.

/// A Lua interpreter version the generated code can target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LuaTarget {
    Lua51,
    Lua52,
    Lua53,
    Lua54,
}

impl LuaTarget {
    /// Parses a target name such as `lua5.1` or `lua54`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "lua5.1" | "lua51" | "5.1" => Some(LuaTarget::Lua51),
            "lua5.2" | "lua52" | "5.2" => Some(LuaTarget::Lua52),
            "lua5.3" | "lua53" | "5.3" => Some(LuaTarget::Lua53),
            "lua5.4" | "lua54" | "5.4" => Some(LuaTarget::Lua54),
            _ => None,
        }
    }

    /// The canonical directory name for the target.
    pub fn name(self) -> &'static str {
        match self {
            LuaTarget::Lua51 => "lua5.1",
            LuaTarget::Lua52 => "lua5.2",
            LuaTarget::Lua53 => "lua5.3",
            LuaTarget::Lua54 => "lua5.4",
        }
    }

    /// Rewrites generated Lua for this target.
    pub fn transform(self, source: &str) -> String {
        match self {
            // Lua 5.1 exposes `unpack` as a global instead of `table.unpack`.
            LuaTarget::Lua51 => {
                let mut output = source.replace("table.unpack(", "unpack(");
                // `math.type` and the `//` operator do not exist before 5.3;
                // Tandem never emits them, so no further rewrite is needed.
                output.push_str(LUA51_COMPAT);
                output
            }
            LuaTarget::Lua52 | LuaTarget::Lua53 | LuaTarget::Lua54 => source.to_string(),
        }
    }
}

/// A small compatibility shim appended for Lua 5.1 targets.
const LUA51_COMPAT: &str = r#"
-- Tandem Lua 5.1 compatibility shim.
if table.unpack == nil then
    table.unpack = unpack
end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_target_names() {
        assert_eq!(LuaTarget::parse("lua5.1"), Some(LuaTarget::Lua51));
        assert_eq!(LuaTarget::parse("LUA54"), Some(LuaTarget::Lua54));
        assert_eq!(LuaTarget::parse("5.3"), Some(LuaTarget::Lua53));
        assert_eq!(LuaTarget::parse("luajit"), None);
    }

    #[test]
    fn lua51_rewrites_table_unpack() {
        let output = LuaTarget::Lua51.transform("local a = table.unpack(x)");
        assert!(output.contains("local a = unpack(x)"));
        assert!(!output.contains("table.unpack(x)"));
        assert!(output.contains("Lua 5.1 compatibility shim"));
    }

    #[test]
    fn lua54_leaves_source_unchanged() {
        let source = "local a = table.unpack(x)";
        assert_eq!(LuaTarget::Lua54.transform(source), source);
    }
}
