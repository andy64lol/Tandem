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
    signature("error", &[Ty::Any, Ty::Int], 1, Ty::Void),
    signature("getmetatable", &[Ty::Any], 1, Ty::Any),
    signature("ipairs", &[Ty::Any], 1, Ty::Any),
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
    variadic("char", &[], 0, Ty::Str),
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
        Ty::Any,
    ),
    variadic("pack", &[], 0, Ty::Any),
    signature("remove", &[Ty::Any, Ty::Int], 1, Ty::Any),
    signature("sort", &[Ty::Any, Ty::Any], 1, Ty::Void),
    signature("unpack", &[Ty::Any, Ty::Int, Ty::Int], 1, Ty::Any),
];

const MATH_FUNCTIONS: &[Signature] = &[
    signature("acos", &[Ty::Any], 1, Ty::Float),
    signature("asin", &[Ty::Any], 1, Ty::Float),
    signature("atan", &[Ty::Any, Ty::Any], 1, Ty::Float),
    signature("ceil", &[Ty::Any], 1, Ty::Int),
    signature("cos", &[Ty::Any], 1, Ty::Float),
    signature("deg", &[Ty::Any], 1, Ty::Float),
    signature("exp", &[Ty::Any], 1, Ty::Float),
    signature("floor", &[Ty::Any], 1, Ty::Int),
    signature("log", &[Ty::Any, Ty::Any], 1, Ty::Float),
    signature("rad", &[Ty::Any], 1, Ty::Float),
    signature("randomseed", &[Ty::Any, Ty::Any], 0, Ty::Void),
    signature("sin", &[Ty::Any], 1, Ty::Float),
    signature("sqrt", &[Ty::Any], 1, Ty::Float),
    signature("tan", &[Ty::Any], 1, Ty::Float),
    signature("ult", &[Ty::Any, Ty::Any], 2, Ty::Bool),
];

const OS_FUNCTIONS: &[Signature] = &[
    signature("clock", &[], 0, Ty::Float),
    signature("date", &[Ty::Str, Ty::Int], 0, Ty::Str),
    signature("difftime", &[Ty::Any, Ty::Any], 2, Ty::Float),
    signature("getenv", &[Ty::Str], 1, Ty::Any),
    signature("time", &[Ty::Any], 0, Ty::Int),
    signature("tmpname", &[], 0, Ty::Str),
];

const COROUTINE_FUNCTIONS: &[Signature] = &[
    signature("create", &[Ty::Any], 1, Ty::Any),
    signature("isyieldable", &[], 0, Ty::Bool),
    variadic("resume", &[Ty::Any], 1, Ty::Bool),
    signature("status", &[Ty::Any], 1, Ty::Str),
    signature("wrap", &[Ty::Any], 1, Ty::Any),
    variadic("yield", &[], 0, Ty::Any),
];

const UTF8_FUNCTIONS: &[Signature] = &[
    variadic("char", &[], 0, Ty::Str),
    signature("codepoint", &[Ty::Str, Ty::Int, Ty::Int], 1, Ty::Int),
    signature("len", &[Ty::Str, Ty::Int, Ty::Int], 1, Ty::Int),
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
        functions: &[],
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
        functions: &[],
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
        functions: &[],
        values: &[("config", Ty::Str), ("cpath", Ty::Str), ("path", Ty::Str)],
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
