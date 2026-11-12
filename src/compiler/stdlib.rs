//! Static signatures for the Lua 5.4 standard library.
//!
//! The tables below follow the Lua 5.4 reference manual
//! (<https://www.lua.org/manual/5.4/manual.html>) so that the checker can
//! validate calls into the standard library instead of falling back to a
//! dynamic `Any` result.
//!
//! Conventions used throughout this file:
//!
//! * Numeric arguments are typed `Ty::Any`: Lua accepts both integers and
//!   floats for them while Tandem keeps the two types apart.
//! * Results that are unambiguous in the manual use their exact type
//!   (`math.floor` is an `Int`, `math.sqrt` is a `Float`, `table.concat` is a
//!   `Str`, `io.write` produces no value, ...).
//! * Results that depend on the runtime value, or that yield several values
//!   at once, are typed `Ty::Any` (`io.read`, `os.getenv`, `tonumber`,
//!   `string.match`, ...).

use crate::compiler::ast::Ty;

pub(crate) struct Signature {
    pub(crate) name: &'static str,
    pub(crate) args: &'static [Ty],
    pub(crate) required: usize,
    pub(crate) ret: Ty,
    pub(crate) varargs: bool,
}

pub(crate) struct Package {
    pub(crate) name: &'static str,
    pub(crate) members: &'static [&'static str],
    pub(crate) functions: &'static [Signature],
    pub(crate) values: &'static [(&'static str, Ty)],
}

const fn signature(name: &'static str, args: &'static [Ty], required: usize, ret: Ty) -> Signature {
    Signature {
        name,
        args,
        required,
        ret,
        varargs: false,
    }
}

const fn variadic(name: &'static str, args: &'static [Ty], required: usize, ret: Ty) -> Signature {
    Signature {
        name,
        args,
        required,
        ret,
        varargs: true,
    }
}

pub(crate) const BASE_FUNCTIONS: &[&str] = &[
    "assert",
    "collectgarbage",
    "dofile",
    "error",
    "getmetatable",
    "ipairs",
    "load",
    "loadfile",
    "next",
    "pairs",
    "pcall",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "warn",
    "xpcall",
];

pub(crate) const BASE_VALUES: &[&str] = &["_G", "_VERSION"];

pub(crate) const BASE_SIGNATURES: &[Signature] = &[
    signature("assert", &[Ty::Any, Ty::Any], 1, Ty::Any),
    signature("collectgarbage", &[Ty::Str, Ty::Int], 0, Ty::Int),
    signature("dofile", &[Ty::Str], 0, Ty::Any),
    signature("error", &[Ty::Any, Ty::Int], 1, Ty::Void),
    signature("getmetatable", &[Ty::Any], 1, Ty::Any),
    signature("ipairs", &[Ty::Any], 1, Ty::Any),
    signature("load", &[Ty::Any, Ty::Str, Ty::Str, Ty::Any], 1, Ty::Any),
    signature("loadfile", &[Ty::Str, Ty::Str, Ty::Any], 0, Ty::Any),
    signature("next", &[Ty::Any, Ty::Any], 1, Ty::Any),
    signature("pairs", &[Ty::Any], 1, Ty::Any),
    variadic("pcall", &[Ty::Any], 1, Ty::Bool),
    signature("rawequal", &[Ty::Any, Ty::Any], 2, Ty::Bool),
    signature("rawget", &[Ty::Any, Ty::Any], 2, Ty::Any),
    signature("rawlen", &[Ty::Any], 1, Ty::Int),
    signature("rawset", &[Ty::Any, Ty::Any, Ty::Any], 3, Ty::Any),
    variadic("select", &[Ty::Any], 1, Ty::Any),
    signature("setmetatable", &[Ty::Any, Ty::Any], 2, Ty::Any),
    signature("tonumber", &[Ty::Any, Ty::Int], 1, Ty::Any),
    signature("tostring", &[Ty::Any], 1, Ty::Str),
    signature("type", &[Ty::Any], 1, Ty::Str),
    variadic("warn", &[Ty::Any], 1, Ty::Void),
    variadic("xpcall", &[Ty::Any, Ty::Any], 2, Ty::Bool),
];

const STRING_FUNCTIONS: &[Signature] = &[
    signature("byte", &[Ty::Str, Ty::Int, Ty::Int], 1, Ty::Int),
    variadic("char", &[Ty::Int], 1, Ty::Str),
    signature("dump", &[Ty::Any], 1, Ty::Str),
    signature("find", &[Ty::Str, Ty::Str, Ty::Int, Ty::Bool], 2, Ty::Any),
    variadic("format", &[Ty::Str], 1, Ty::Str),
    signature("gmatch", &[Ty::Str, Ty::Str], 2, Ty::Any),
    signature("gsub", &[Ty::Str, Ty::Str, Ty::Any, Ty::Int], 3, Ty::Str),
    signature("len", &[Ty::Str], 1, Ty::Int),
    signature("lower", &[Ty::Str], 1, Ty::Str),
    signature("match", &[Ty::Str, Ty::Str, Ty::Int], 2, Ty::Any),
    variadic("pack", &[Ty::Str], 1, Ty::Str),
    signature("packsize", &[Ty::Str], 1, Ty::Int),
    signature("rep", &[Ty::Str, Ty::Int, Ty::Str], 2, Ty::Str),
    signature("reverse", &[Ty::Str], 1, Ty::Str),
    signature("sub", &[Ty::Str, Ty::Int, Ty::Int], 2, Ty::Str),
    signature("unpack", &[Ty::Str, Ty::Str, Ty::Int], 2, Ty::Any),
    signature("upper", &[Ty::Str], 1, Ty::Str),
];

const TABLE_FUNCTIONS: &[Signature] = &[
    signature("concat", &[Ty::Any, Ty::Str, Ty::Int, Ty::Int], 1, Ty::Str),
    variadic("insert", &[Ty::Any, Ty::Any], 2, Ty::Void),
    signature(
        "move",
        &[Ty::Any, Ty::Int, Ty::Int, Ty::Int, Ty::Any],
        4,
        Ty::Table,
    ),
    variadic("pack", &[], 0, Ty::Table),
    signature("remove", &[Ty::Any, Ty::Int], 1, Ty::Any),
    signature("sort", &[Ty::Any, Ty::Any], 1, Ty::Void),
    signature("unpack", &[Ty::Any, Ty::Int, Ty::Int], 1, Ty::Any),
];

const MATH_FUNCTIONS: &[Signature] = &[
    signature("abs", &[Ty::Any], 1, Ty::Any),
    signature("acos", &[Ty::Any], 1, Ty::Float),
    signature("asin", &[Ty::Any], 1, Ty::Float),
    signature("atan", &[Ty::Any, Ty::Any], 1, Ty::Float),
    signature("ceil", &[Ty::Any], 1, Ty::Int),
    signature("cos", &[Ty::Any], 1, Ty::Float),
    signature("deg", &[Ty::Any], 1, Ty::Float),
    signature("exp", &[Ty::Any], 1, Ty::Float),
    signature("floor", &[Ty::Any], 1, Ty::Int),
    signature("fmod", &[Ty::Any, Ty::Any], 2, Ty::Float),
    signature("log", &[Ty::Any, Ty::Any], 1, Ty::Float),
    variadic("max", &[Ty::Any], 1, Ty::Any),
    variadic("min", &[Ty::Any], 1, Ty::Any),
    signature("modf", &[Ty::Any], 1, Ty::Any),
    signature("rad", &[Ty::Any], 1, Ty::Float),
    variadic("random", &[Ty::Any, Ty::Any], 0, Ty::Any),
    signature("randomseed", &[Ty::Any, Ty::Any], 0, Ty::Void),
    signature("sin", &[Ty::Any], 1, Ty::Float),
    signature("sqrt", &[Ty::Any], 1, Ty::Float),
    signature("tan", &[Ty::Any], 1, Ty::Float),
    signature("tointeger", &[Ty::Any], 1, Ty::Any),
    signature("type", &[Ty::Any], 1, Ty::Str),
    signature("ult", &[Ty::Any, Ty::Any], 2, Ty::Bool),
];

const OS_FUNCTIONS: &[Signature] = &[
    signature("clock", &[], 0, Ty::Float),
    signature("date", &[Ty::Str, Ty::Int], 0, Ty::Str),
    signature("difftime", &[Ty::Any, Ty::Any], 2, Ty::Float),
    signature("execute", &[Ty::Str], 0, Ty::Bool),
    signature("exit", &[Ty::Any, Ty::Any], 0, Ty::Void),
    signature("getenv", &[Ty::Str], 1, Ty::Any),
    signature("remove", &[Ty::Str], 1, Ty::Bool),
    signature("rename", &[Ty::Str, Ty::Str], 2, Ty::Bool),
    signature("setlocale", &[Ty::Str, Ty::Str], 1, Ty::Any),
    signature("time", &[Ty::Any], 0, Ty::Int),
    signature("tmpname", &[], 0, Ty::Str),
];

const IO_FUNCTIONS: &[Signature] = &[
    signature("close", &[Ty::Any], 0, Ty::Bool),
    signature("flush", &[], 0, Ty::Void),
    signature("input", &[Ty::Any], 0, Ty::Any),
    variadic("lines", &[Ty::Str], 0, Ty::Any),
    signature("open", &[Ty::Str, Ty::Str], 1, Ty::Any),
    signature("output", &[Ty::Any], 0, Ty::Any),
    signature("popen", &[Ty::Str, Ty::Str], 1, Ty::Any),
    variadic("read", &[Ty::Any], 0, Ty::Any),
    signature("tmpfile", &[], 0, Ty::Any),
    signature("type", &[Ty::Any], 1, Ty::Str),
    variadic("write", &[Ty::Any], 0, Ty::Void),
];

const COROUTINE_FUNCTIONS: &[Signature] = &[
    signature("close", &[Ty::Any], 1, Ty::Bool),
    signature("create", &[Ty::Any], 1, Ty::Any),
    signature("isyieldable", &[], 0, Ty::Bool),
    variadic("resume", &[Ty::Any], 1, Ty::Bool),
    signature("running", &[], 0, Ty::Any),
    signature("status", &[Ty::Any], 1, Ty::Str),
    signature("wrap", &[Ty::Any], 1, Ty::Any),
    variadic("yield", &[], 0, Ty::Any),
];

const UTF8_FUNCTIONS: &[Signature] = &[
    variadic("char", &[Ty::Int], 1, Ty::Str),
    signature("codepoint", &[Ty::Str, Ty::Int, Ty::Int], 1, Ty::Int),
    signature("codes", &[Ty::Str], 1, Ty::Any),
    signature("len", &[Ty::Str, Ty::Int, Ty::Int], 1, Ty::Int),
    signature("offset", &[Ty::Str, Ty::Int, Ty::Int], 2, Ty::Int),
];

const DEBUG_FUNCTIONS: &[Signature] = &[
    signature("debug", &[], 0, Ty::Void),
    signature("gethook", &[Ty::Any], 0, Ty::Any),
    signature("getinfo", &[Ty::Any, Ty::Any, Ty::Str], 0, Ty::Any),
    signature("getlocal", &[Ty::Any, Ty::Any, Ty::Any], 1, Ty::Any),
    signature("getmetatable", &[Ty::Any], 1, Ty::Any),
    signature("getregistry", &[], 0, Ty::Any),
    signature("getupvalue", &[Ty::Any, Ty::Int], 2, Ty::Any),
    signature("getuservalue", &[Ty::Any, Ty::Int], 1, Ty::Any),
    signature("sethook", &[Ty::Any, Ty::Any, Ty::Any, Ty::Any], 0, Ty::Void),
    signature("setlocal", &[Ty::Any, Ty::Any, Ty::Any, Ty::Any], 2, Ty::Void),
    signature("setmetatable", &[Ty::Any, Ty::Any], 2, Ty::Any),
    signature("setupvalue", &[Ty::Any, Ty::Int, Ty::Any], 3, Ty::Any),
    signature("setuservalue", &[Ty::Any, Ty::Any, Ty::Int], 2, Ty::Any),
    signature("traceback", &[Ty::Any, Ty::Any, Ty::Any], 0, Ty::Str),
    signature("upvalueid", &[Ty::Any, Ty::Int], 2, Ty::Any),
    signature("upvaluejoin", &[Ty::Any, Ty::Int, Ty::Any, Ty::Int], 4, Ty::Void),
];

const PACKAGE_FUNCTIONS: &[Signature] = &[
    signature("loadlib", &[Ty::Str, Ty::Str], 2, Ty::Any),
    signature("searchpath", &[Ty::Str, Ty::Str, Ty::Str, Ty::Str], 2, Ty::Any),
];

pub(crate) const PACKAGES: &[Package] = &[
    Package {
        name: "string",
        members: &[
            "byte", "char", "dump", "find", "format", "gmatch", "gsub", "len", "lower", "match",
            "pack", "packsize", "rep", "reverse", "sub", "unpack", "upper",
        ],
        functions: STRING_FUNCTIONS,
        values: &[],
    },
    Package {
        name: "table",
        members: &[
            "concat", "insert", "move", "pack", "remove", "sort", "unpack",
        ],
        functions: TABLE_FUNCTIONS,
        values: &[],
    },
    Package {
        name: "math",
        members: &[
            "abs",
            "acos",
            "asin",
            "atan",
            "ceil",
            "cos",
            "deg",
            "exp",
            "floor",
            "fmod",
            "huge",
            "log",
            "max",
            "maxinteger",
            "min",
            "mininteger",
            "modf",
            "pi",
            "rad",
            "random",
            "randomseed",
            "sin",
            "sqrt",
            "tan",
            "tointeger",
            "type",
            "ult",
        ],
        functions: MATH_FUNCTIONS,
        values: &[
            ("huge", Ty::Float),
            ("maxinteger", Ty::Int),
            ("mininteger", Ty::Int),
            ("pi", Ty::Float),
        ],
    },
    Package {
        name: "io",
        members: &[
            "close", "flush", "input", "lines", "open", "output", "popen", "read", "stderr",
            "stdin", "stdout", "tmpfile", "type", "write",
        ],
        functions: IO_FUNCTIONS,
        values: &[("stderr", Ty::Any), ("stdin", Ty::Any), ("stdout", Ty::Any)],
    },
    Package {
        name: "os",
        members: &[
            "clock",
            "date",
            "difftime",
            "execute",
            "exit",
            "getenv",
            "remove",
            "rename",
            "setlocale",
            "time",
            "tmpname",
        ],
        functions: OS_FUNCTIONS,
        values: &[],
    },
    Package {
        name: "coroutine",
        members: &[
            "close",
            "create",
            "isyieldable",
            "resume",
            "running",
            "status",
            "wrap",
            "yield",
        ],
        functions: COROUTINE_FUNCTIONS,
        values: &[],
    },
    Package {
        name: "utf8",
        members: &["char", "charpattern", "codepoint", "codes", "len", "offset"],
        functions: UTF8_FUNCTIONS,
        values: &[("charpattern", Ty::Str)],
    },
    Package {
        name: "debug",
        members: &[
            "debug",
            "gethook",
            "getinfo",
            "getlocal",
            "getmetatable",
            "getregistry",
            "getupvalue",
            "getuservalue",
            "sethook",
            "setlocal",
            "setmetatable",
            "setupvalue",
            "setuservalue",
            "traceback",
            "upvalueid",
            "upvaluejoin",
        ],
        functions: DEBUG_FUNCTIONS,
        values: &[],
    },
    Package {
        name: "package",
        members: &[
            "config",
            "cpath",
            "loaded",
            "loadlib",
            "path",
            "preload",
            "searchers",
            "searchpath",
        ],
        functions: PACKAGE_FUNCTIONS,
        values: &[
            ("config", Ty::Str),
            ("cpath", Ty::Str),
            ("loaded", Ty::Table),
            ("path", Ty::Str),
            ("preload", Ty::Table),
            ("searchers", Ty::Table),
        ],
    },
];

pub(crate) fn package(name: &str) -> Option<&'static Package> {
    PACKAGES.iter().find(|package| package.name == name)
}

pub(crate) fn base_signature(name: &str) -> Option<&'static Signature> {
    BASE_SIGNATURES
        .iter()
        .find(|signature| signature.name == name)
}

pub(crate) fn base_value(name: &str) -> Option<Ty> {
    match name {
        "_G" => Some(Ty::Any),
        "_VERSION" => Some(Ty::Str),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_function(package_name: &str, member: &str) -> Option<&'static Signature> {
        package(package_name)?
            .functions
            .iter()
            .find(|signature| signature.name == member)
    }

    fn package_value(package_name: &str, member: &str) -> Option<Ty> {
        package(package_name)?
            .values
            .iter()
            .find(|(name, _)| *name == member)
            .map(|(_, ty)| ty.clone())
    }

    #[test]
    fn math_abs_has_a_signature() {
        let signature = package_function("math", "abs").expect("math.abs is typed");
        assert_eq!(signature.args, &[Ty::Any]);
        assert_eq!(signature.required, 1);
        assert!(!signature.varargs);
        assert_eq!(signature.ret, Ty::Any);
    }

    #[test]
    fn io_write_is_variadic_and_returns_nothing() {
        let signature = package_function("io", "write").expect("io.write is typed");
        assert!(signature.varargs);
        assert_eq!(signature.required, 0);
        assert_eq!(signature.ret, Ty::Void);
    }

    #[test]
    fn every_package_member_has_a_type() {
        for package in PACKAGES {
            for member in package.members {
                let typed = package.functions.iter().any(|s| s.name == *member)
                    || package.values.iter().any(|(name, _)| *name == *member);
                assert!(
                    typed,
                    "package '{}' has no type for member '{}'",
                    package.name,
                    member
                );
            }
            for signature in package.functions {
                assert!(
                    package.members.contains(&signature.name),
                    "package '{}' types '{}' but does not list it as a member",
                    package.name,
                    signature.name
                );
                assert!(
                    signature.required <= signature.args.len(),
                    "'{}.{}' requires more arguments than it declares",
                    package.name,
                    signature.name
                );
            }
            for (name, _) in package.values {
                assert!(
                    package.members.contains(name),
                    "package '{}' types value '{}' but does not list it as a member",
                    package.name,
                    name
                );
            }
        }
    }

    #[test]
    fn base_library_functions_all_have_signatures() {
        for name in BASE_FUNCTIONS {
            assert!(
                base_signature(name).is_some(),
                "base function '{name}' has no signature"
            );
        }
        assert!(base_signature("load").is_some());
        assert!(base_signature("loadfile").is_some());
        assert!(base_signature("dofile").is_some());
        assert!(base_signature("collectgarbage").is_some());
        assert_eq!(base_signature("tostring").unwrap().ret, Ty::Str);
        assert_eq!(base_signature("type").unwrap().ret, Ty::Str);
        assert_eq!(base_signature("rawlen").unwrap().ret, Ty::Int);
        assert_eq!(base_signature("pcall").unwrap().ret, Ty::Bool);
        assert!(base_signature("pcall").unwrap().varargs);
    }

    #[test]
    fn precise_return_types_are_used_where_the_manual_is_unambiguous() {
        assert_eq!(package_function("math", "floor").unwrap().ret, Ty::Int);
        assert_eq!(package_function("math", "ceil").unwrap().ret, Ty::Int);
        assert_eq!(package_function("math", "sqrt").unwrap().ret, Ty::Float);
        assert_eq!(package_function("math", "fmod").unwrap().ret, Ty::Float);
        assert_eq!(package_function("math", "type").unwrap().ret, Ty::Str);
        assert_eq!(package_function("string", "len").unwrap().ret, Ty::Int);
        assert_eq!(package_function("string", "byte").unwrap().ret, Ty::Int);
        assert_eq!(package_function("string", "format").unwrap().ret, Ty::Str);
        assert_eq!(package_function("string", "rep").unwrap().ret, Ty::Str);
        assert_eq!(package_function("string", "sub").unwrap().ret, Ty::Str);
        assert_eq!(package_function("string", "reverse").unwrap().ret, Ty::Str);
        assert_eq!(package_function("table", "concat").unwrap().ret, Ty::Str);
        assert_eq!(package_function("os", "time").unwrap().ret, Ty::Int);
        assert_eq!(package_function("os", "clock").unwrap().ret, Ty::Float);
        assert_eq!(package_function("os", "difftime").unwrap().ret, Ty::Float);
        assert_eq!(package_function("utf8", "len").unwrap().ret, Ty::Int);
        assert_eq!(package_function("utf8", "offset").unwrap().ret, Ty::Int);
        assert_eq!(package_function("utf8", "char").unwrap().ret, Ty::Str);
        assert_eq!(package_function("debug", "traceback").unwrap().ret, Ty::Str);
        assert_eq!(package_function("coroutine", "status").unwrap().ret, Ty::Str);
        assert_eq!(package_function("coroutine", "isyieldable").unwrap().ret, Ty::Bool);
        assert_eq!(package_function("io", "type").unwrap().ret, Ty::Str);
        assert_eq!(package_value("math", "pi").unwrap(), Ty::Float);
        assert_eq!(package_value("math", "huge").unwrap(), Ty::Float);
        assert_eq!(package_value("math", "maxinteger").unwrap(), Ty::Int);
        assert_eq!(package_value("math", "mininteger").unwrap(), Ty::Int);
        assert_eq!(package_value("utf8", "charpattern").unwrap(), Ty::Str);
    }

    #[test]
    fn variadic_functions_are_marked_as_such() {
        for member in ["write", "read", "lines"] {
            assert!(
                package_function("io", member).unwrap().varargs,
                "io.{member} should be variadic"
            );
        }
        for member in ["format", "char", "pack"] {
            assert!(
                package_function("string", member).unwrap().varargs,
                "string.{member} should be variadic"
            );
        }
        for member in ["char"] {
            assert!(
                package_function("utf8", member).unwrap().varargs,
                "utf8.{member} should be variadic"
            );
        }
        assert!(package_function("table", "pack").unwrap().varargs);
        assert!(package_function("table", "insert").unwrap().varargs);
        assert!(package_function("coroutine", "resume").unwrap().varargs);
        assert!(package_function("coroutine", "yield").unwrap().varargs);
        assert!(package_function("math", "max").unwrap().varargs);
        assert!(package_function("math", "min").unwrap().varargs);
        assert!(package_function("math", "random").unwrap().varargs);
    }
}
