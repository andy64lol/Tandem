#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Int,
    Float,
    Bool,
    Str,
    Void,
}
impl Ty {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s {
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "void" => Some(Self::Void),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Var(String),
    Unary(String, Box<Expr>),
    Bin(Box<Expr>, String, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FormatPart {
    Text(String),
    Variable(String),
}

pub(crate) fn parse_printf_format(format: &str) -> Result<Vec<FormatPart>, String> {
    let chars: Vec<char> = format.chars().collect();
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut index = 0;

    while index < chars.len() {
        match chars[index] {
            '{' if chars.get(index + 1) == Some(&'{') => {
                text.push('{');
                index += 2;
            }
            '{' => {
                let Some(relative_end) = chars[index + 1..].iter().position(|c| *c == '}') else {
                    return Err("unclosed placeholder in printf string".into());
                };
                let end = index + 1 + relative_end;
                let name: String = chars[index + 1..end].iter().collect();
                let mut name_chars = name.chars();
                let valid_start = name_chars
                    .next()
                    .is_some_and(|c| c == '_' || c.is_ascii_alphabetic());
                if !valid_start || !name_chars.all(|c| c == '_' || c.is_ascii_alphanumeric()) {
                    return Err(format!(
                        "printf placeholder '{{{name}}}' must be a variable name"
                    ));
                }
                if !text.is_empty() {
                    parts.push(FormatPart::Text(std::mem::take(&mut text)));
                }
                parts.push(FormatPart::Variable(name));
                index = end + 1;
            }
            '}' if chars.get(index + 1) == Some(&'}') => {
                text.push('}');
                index += 2;
            }
            '}' => return Err("unmatched '}' in printf string".into()),
            c => {
                text.push(c);
                index += 1;
            }
        }
    }

    if !text.is_empty() || parts.is_empty() {
        parts.push(FormatPart::Text(text));
    }
    Ok(parts)
}

#[derive(Clone, Debug)]
pub(crate) enum Stmt {
    Decl(Ty, String, Expr),
    Assign(String, String, Expr),
    Expr(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    For {
        variable: String,
        start: Expr,
        stop: Expr,
        step: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
}
#[derive(Clone, Debug)]
pub(crate) struct Param {
    pub(crate) ty: Ty,
    pub(crate) name: String,
    pub(crate) default: Option<Expr>,
}
#[derive(Clone, Debug)]
pub(crate) struct Fun {
    pub(crate) name: String,
    pub(crate) params: Vec<Param>,
    pub(crate) ret: Ty,
    pub(crate) body: Vec<Stmt>,
}
#[derive(Clone, Debug)]
pub(crate) enum Top {
    Fun(Fun),
    Stmt(Stmt),
}
