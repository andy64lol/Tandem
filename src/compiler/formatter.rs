//! Source formatter for Tandem (`.tdm`) files.
//!
//! The formatter works on the lexer token stream instead of the AST, so it
//! keeps working when new language constructs are added.  The lexer discards
//! comments, therefore comments are re-scanned from the raw source text and
//! merged back into the token stream before the source is re-printed.
//!
//! The canonical style produced by [`format_source`] is:
//!
//! * four spaces of indentation per block, block bodies on their own lines,
//! * one `;` after every statement,
//! * one space around binary operators and after `,`, none before `,` or `;`,
//! * `if (cond) { ... } else { ... }`, `while (cond) { ... }`, `for (i, 10) { ... }`,
//! * `fn name(int a, int b) -> int { ... }`,
//! * one member per line for class, struct and interface bodies,
//! * string literals are copied byte for byte from the input.

use crate::compiler::lexer::{lex, Tok, Token};

/// The indentation used for a single nesting level.
const INDENT: &str = "    ";

/// Keywords that can never be the end of an expression operand.
const NON_OPERAND_KEYWORDS: &[&str] = &[
    "as",
    "auto",
    "bool",
    "class",
    "const",
    "constructor",
    "dict",
    "else",
    "enum",
    "extends",
    "float",
    "fn",
    "for",
    "if",
    "implements",
    "import",
    "int",
    "interface",
    "list",
    "local",
    "match",
    "mixin",
    "module",
    "new",
    "null",
    "private",
    "return",
    "static",
    "str",
    "struct",
    "super",
    "table",
    "trait",
    "tuple",
    "union",
    "void",
    "while",
];

/// Keywords that are followed by a space and an opening parenthesis.
const PAREN_KEYWORDS: &[&str] = &["if", "while", "for", "return"];

/// Keywords that are never a type name.
const NON_TYPE_KEYWORDS: &[&str] = &[
    "as",
    "class",
    "const",
    "constructor",
    "else",
    "enum",
    "extends",
    "false",
    "fn",
    "for",
    "if",
    "implements",
    "import",
    "instanceof",
    "interface",
    "local",
    "match",
    "mixin",
    "new",
    "private",
    "return",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "union",
    "while",
];

/// Formats a Tandem source file.
///
/// Returns the formatted source on success, or a human readable message that
/// describes the problem (for example a lexer error) on failure.
pub fn format_source(source: &str) -> Result<String, String> {
    let tokens = lex(source)?;
    let items = collect_items(source, &tokens);
    let mut printer = Printer::new();
    printer.print(&items);
    Ok(align_trailing_comments(printer.finish()))
}

/// Finds the byte offset of a trailing line comment, ignoring strings.
fn trailing_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active) = quote {
            if byte == b'\\' {
                index += 2;
                continue;
            }
            if byte == active {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' => quote = Some(byte),
            b'/' if bytes.get(index + 1) == Some(&b'/') => return Some(index),
            b'-' if bytes.get(index + 1) == Some(&b'-') => return Some(index),
            _ => {}
        }
        index += 1;
    }
    None
}

/// Aligns runs of consecutive trailing comments to a common column.
fn align_trailing_comments(mut output: String) -> String {
    let mut lines: Vec<String> = output.lines().map(str::to_string).collect();
    let mut index = 0;
    while index < lines.len() {
        if trailing_comment(&lines[index]).is_none() {
            index += 1;
            continue;
        }
        let start = index;
        while index < lines.len() && trailing_comment(&lines[index]).is_some() {
            index += 1;
        }
        if index - start < 2 {
            continue;
        }
        let width = (start..index)
            .filter_map(|line| trailing_comment(&lines[line]).map(|column| lines[line][..column].trim_end().len()))
            .max()
            .unwrap_or(0);
        for line in &mut lines[start..index] {
            if let Some(column) = trailing_comment(line) {
                let code = line[..column].trim_end().to_string();
                let comment = line[column..].to_string();
                *line = format!("{code}{}{comment}", " ".repeat(width - code.len() + 1));
            }
        }
    }
    output = lines.join("\n");
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Maps byte offsets to 1-based line numbers.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset + 1);
            }
        }
        Self { starts }
    }

    fn line_of(&self, position: usize) -> usize {
        match self.starts.binary_search(&position) {
            Ok(index) => index + 1,
            Err(index) => index,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    Line,
    Block,
}

struct Comment {
    kind: CommentKind,
    text: String,
    line: usize,
    end_line: usize,
    own_line: bool,
}

struct TokItem {
    kind: Tok,
    text: String,
    line: usize,
    end_line: usize,
    /// True when the next token opens a generic argument list.
    generic_args_follow: bool,
    /// The number of type atoms that follow this identifier (`list`/`table`).
    type_atoms_follow: usize,
}

enum Item {
    Token(TokItem),
    Comment(Comment),
}

#[derive(Clone)]
enum Prev {
    Ident(String),
    Num,
    Str,
    Sym(String),
}

struct Brace {
    kind: BraceKind,
    empty: bool,
    paren_depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BraceKind {
    /// A statement block: `if (x) { ... }`.
    Block,
    /// A table or list literal: `{ a = 1 }`.
    Table,
    /// A block whose members are separated by commas: `enum Color { Red, Green }`.
    List,
    /// A match expression: `match x { 1 => "one" }`.
    Match,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Normal,
    GenericOpen,
    GenericClose,
}

/// The token stream pretty printer.
struct Printer {
    lines: Vec<String>,
    current: String,
    indent: usize,
    /// True when the next content has to start on a new line.
    pending_break: bool,
    started: bool,
    last_end_line: usize,
    prev: Option<Prev>,
    /// True when the previous token must not be followed by a space.
    prev_tight: bool,
    /// True when the previous token was the `>` of a generic argument list.
    prev_generic_close: bool,
    braces: Vec<Brace>,
    paren_depth: usize,
    /// The parenthesis depth at which a block body is expected.
    expect_block_depth: Option<usize>,
    /// The parenthesis depth at which a comma separated body is expected.
    expect_list_depth: Option<usize>,
    /// The parenthesis depth at which a declaration body is expected.
    expect_declaration_depth: Option<usize>,
    generic_depth: usize,
    expect_generic: bool,
    /// The number of type atoms that are still expected.
    type_needed: usize,
    fn_header: bool,
    class_header: bool,
    last_close_block: bool,
}

impl Printer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current: String::new(),
            indent: 0,
            pending_break: false,
            started: false,
            last_end_line: 0,
            prev: None,
            prev_tight: false,
            prev_generic_close: false,
            braces: Vec::new(),
            paren_depth: 0,
            expect_block_depth: None,
            expect_list_depth: None,
            expect_declaration_depth: None,
            generic_depth: 0,
            expect_generic: false,
            type_needed: 0,
            fn_header: false,
            class_header: false,
            last_close_block: false,
        }
    }

    fn print(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Token(token) => self.token(token),
                Item::Comment(comment) => self.comment(comment),
            }
        }
        self.flush();
    }

    fn finish(self) -> String {
        let mut out = self.lines.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        out
    }

    fn indent_string(&self) -> String {
        INDENT.repeat(self.indent)
    }

    /// Marks the end of a line.  The line is only pushed once the next content
    /// is written, so that a trailing comment can still join it.
    fn end_line(&mut self) {
        self.pending_break = true;
    }

    /// Pushes the current line and starts a new one, keeping at most one blank
    /// line when the input had one.
    fn break_line(&mut self, line: usize) {
        if !self.pending_break {
            return;
        }
        self.pending_break = false;
        let current = std::mem::take(&mut self.current);
        if !current.trim().is_empty() {
            self.lines.push(current);
        }
        if self.started && line > self.last_end_line + 1 {
            self.lines.push(String::new());
        }
        self.current = self.indent_string();
    }

    fn write_text(&mut self, text: &str, space: bool, line: usize) {
        self.break_line(line);
        if space && !self.current.trim().is_empty() {
            self.current.push(' ');
        }
        self.current.push_str(text);
    }

    fn write_token(&mut self, item: &TokItem, space: bool) {
        self.write_text(&item.text, space, item.line);
        if let Some(frame) = self.braces.last_mut() {
            if frame.kind == BraceKind::Table {
                frame.empty = false;
            }
        }
        self.last_end_line = item.end_line;
        self.started = true;
    }

    fn flush(&mut self) {
        self.pending_break = false;
        let current = std::mem::take(&mut self.current);
        if !current.trim().is_empty() {
            self.lines.push(current);
        }
    }

    fn token(&mut self, item: &TokItem) {
        if self.last_close_block {
            self.last_close_block = false;
            let is_else = matches!(&item.kind, Tok::Ident(name) if name == "else");
            if !is_else {
                self.end_line();
            }
        }
        let role = self.classify(item);
        let space = self.space_before(item, role);
        match &item.kind {
            Tok::Ident(name) => {
                self.handle_ident(name, item);
                self.write_token(item, space);
            }
            Tok::Sym(symbol) => match symbol.as_str() {
                "{" => self.open_brace(item, space),
                "}" => self.close_brace(item),
                ";" => {
                    self.write_token(item, space);
                    self.end_line();
                    self.expect_block_depth = None;
                    self.expect_declaration_depth = None;
                    self.type_needed = 1;
                }
                "," => {
                    self.write_token(item, space);
                    if self.in_comma_block() {
                        self.end_line();
                    }
                }
                "(" => {
                    self.write_token(item, space);
                    self.paren_depth += 1;
                }
                ")" => {
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                    self.write_token(item, space);
                }
                "->" => {
                    self.write_token(item, space);
                    self.type_needed = 1;
                }
                _ => self.write_token(item, space),
            },
            _ => self.write_token(item, space),
        }
        self.update_prev(item, role);
    }

    /// Decides whether a `<` opens a generic argument list and whether a `>`
    /// closes one.
    fn classify(&mut self, item: &TokItem) -> Role {
        match &item.kind {
            Tok::Sym(symbol) if symbol == "<" => {
                if self.expect_generic || self.type_needed > 0 {
                    self.expect_generic = false;
                    self.generic_depth += 1;
                    Role::GenericOpen
                } else {
                    Role::Normal
                }
            }
            Tok::Sym(symbol) if symbol == ">" => {
                if self.generic_depth > 0 {
                    self.generic_depth -= 1;
                    Role::GenericClose
                } else {
                    Role::Normal
                }
            }
            _ => {
                self.expect_generic = false;
                Role::Normal
            }
        }
    }

    /// Computes whether a space is printed before the given token.
    fn space_before(&self, item: &TokItem, role: Role) -> bool {
        if self.pending_break || self.current.trim().is_empty() {
            return false;
        }
        let Some(prev) = &self.prev else {
            return false;
        };
        match &item.kind {
            Tok::Sym(symbol) => match symbol.as_str() {
                "," | ";" | ")" | "]" | "." => false,
                "?" => false,
                "(" => {
                    if let Prev::Ident(name) = prev {
                        if !PAREN_KEYWORDS.contains(&name.as_str()) {
                            return false;
                        }
                    }
                    if let Prev::Sym(symbol) = prev {
                        if symbol == ">" && self.prev_generic_close {
                            return false;
                        }
                    }
                    !self.prev_tight
                }
                "[" => !prev_ends_operand(prev),
                "<" => role != Role::GenericOpen && !self.prev_tight,
                ">" => role != Role::GenericClose && !self.prev_tight,
                _ => !self.prev_tight,
            },
            _ => !self.prev_tight,
        }
    }

    fn update_prev(&mut self, item: &TokItem, role: Role) {
        let was_operand = self.prev.as_ref().map(prev_ends_operand).unwrap_or(false);
        self.prev = Some(match &item.kind {
            Tok::Ident(name) => Prev::Ident(name.clone()),
            Tok::Num(_) => Prev::Num,
            Tok::Str(_) => Prev::Str,
            Tok::Sym(symbol) => Prev::Sym(symbol.clone()),
            Tok::Eof => return,
        });
        self.prev_tight = match &item.kind {
            Tok::Sym(symbol) if symbol == "(" || symbol == "[" || symbol == "." => true,
            Tok::Sym(symbol) if symbol == "!" || symbol == "&" => true,
            Tok::Sym(symbol) if symbol == "-" => !was_operand,
            Tok::Sym(symbol) if symbol == "<" && role == Role::GenericOpen => true,
            _ => false,
        };
        self.prev_generic_close = role == Role::GenericClose;
    }

    /// Tracks the keywords that change how the following tokens are printed.
    fn handle_ident(&mut self, name: &str, item: &TokItem) {
        let is_header_name = self.fn_header || self.class_header;
        self.fn_header = false;
        self.class_header = false;
        match name {
            "fn" => {
                self.fn_header = true;
                self.expect_block_depth = Some(self.paren_depth);
            }
            "class" | "interface" | "struct" => {
                self.class_header = true;
                self.expect_block_depth = Some(self.paren_depth);
            }
            "constructor" => {
                self.expect_block_depth = Some(self.paren_depth);
            }
            "match" => {
                self.expect_block_depth = Some(self.paren_depth);
                self.expect_list_depth = Some(self.paren_depth);
            }
            "enum" | "union" => {
                self.expect_block_depth = Some(self.paren_depth);
                self.expect_list_depth = Some(self.paren_depth);
                self.expect_declaration_depth = Some(self.paren_depth);
            }
            "new" | "as" => {
                self.type_needed = 1;
            }
            _ => {}
        }
        if is_header_name && item.generic_args_follow {
            self.expect_generic = true;
        }
        if !NON_TYPE_KEYWORDS.contains(&name) {
            self.note_type_atom(name, item);
        }
    }

    /// Keeps track of how many type atoms are still expected.
    fn note_type_atom(&mut self, name: &str, item: &TokItem) {
        if self.type_needed == 0 {
            return;
        }
        match name {
            "list" => self.type_needed = 1,
            "table" => self.type_needed = item.type_atoms_follow,
            _ => {
                self.type_needed -= 1;
                if self.type_needed == 0 && item.generic_args_follow {
                    self.expect_generic = true;
                }
            }
        }
    }

    /// Decides whether a `{` opens a block or a table literal.
    fn decide_block(&mut self) -> bool {
        if let Some(depth) = self.expect_block_depth {
            if depth == self.paren_depth {
                self.expect_block_depth = None;
                return true;
            }
        }
        match &self.prev {
            None => true,
            Some(Prev::Sym(symbol)) => match symbol.as_str() {
                ")" | "else" | ";" | "}" => true,
                "{" => matches!(
                    self.braces.last().map(|frame| frame.kind),
                    Some(BraceKind::Block) | Some(BraceKind::List) | Some(BraceKind::Match)
                ),
                _ => false,
            },
            Some(Prev::Ident(_)) => true,
            _ => false,
        }
    }

    /// True when a comma separates two members of the innermost comma block.
    fn in_comma_block(&self) -> bool {
        match self.braces.last() {
            Some(frame) => {
                matches!(frame.kind, BraceKind::List | BraceKind::Match)
                    && frame.paren_depth == self.paren_depth
            }
            None => false,
        }
    }

    fn open_brace(&mut self, item: &TokItem, space: bool) {
        let is_block = self.decide_block();
        let is_list = self.expect_list_depth == Some(self.paren_depth);
        if is_list {
            self.expect_list_depth = None;
        }
        let is_declaration = self.expect_declaration_depth == Some(self.paren_depth);
        if is_declaration {
            self.expect_declaration_depth = None;
        }
        self.write_token(item, space);
        let kind = if is_list {
            if is_declaration {
                BraceKind::List
            } else {
                BraceKind::Match
            }
        } else if is_block {
            BraceKind::Block
        } else {
            BraceKind::Table
        };
        self.braces.push(Brace {
            kind,
            empty: true,
            paren_depth: self.paren_depth,
        });
        if kind != BraceKind::Table {
            self.indent += 1;
            self.end_line();
            self.type_needed = 1;
        }
    }

    fn close_brace(&mut self, item: &TokItem) {
        let frame = self.braces.pop();
        let kind = frame.as_ref().map(|frame| frame.kind);
        if matches!(
            kind,
            Some(BraceKind::Block) | Some(BraceKind::List) | Some(BraceKind::Match)
        ) {
            self.indent = self.indent.saturating_sub(1);
            self.end_line();
            self.write_token(item, false);
            self.type_needed = 1;
            self.expect_block_depth = None;
            // A match expression is a value, so a `;` may still follow it.
            self.last_close_block = kind != Some(BraceKind::Match);
        } else {
            let empty = frame.as_ref().is_none_or(|frame| frame.empty);
            self.write_token(item, !empty);
            self.last_close_block = false;
        }
    }

    fn comment(&mut self, comment: &Comment) {
        if comment.own_line {
            self.last_close_block = false;
            if !self.current.trim().is_empty() {
                self.pending_break = true;
            }
            self.break_line(comment.line);
            self.write_comment_text(comment);
            self.pending_break = true;
        } else {
            if !self.current.trim().is_empty() {
                self.current.push(' ');
            }
            self.write_comment_text(comment);
            if comment.kind == CommentKind::Line {
                self.pending_break = true;
            }
        }
        self.last_end_line = comment.end_line;
        self.started = true;
    }

    fn write_comment_text(&mut self, comment: &Comment) {
        let mut parts = comment.text.split('\n');
        let first = parts.next().unwrap_or("");
        self.current.push_str(first);
        for part in parts {
            let current = std::mem::take(&mut self.current);
            if !current.trim().is_empty() {
                self.lines.push(current);
            }
            self.current = self.indent_string();
            self.current.push_str(part.trim());
        }
    }
}

fn prev_ends_operand(prev: &Prev) -> bool {
    match prev {
        Prev::Num | Prev::Str => true,
        Prev::Ident(name) => !NON_OPERAND_KEYWORDS.contains(&name.as_str()),
        Prev::Sym(symbol) => matches!(symbol.as_str(), ")" | "]" | "}"),
    }
}

/// Collects the tokens and the comments of a source file in source order.
fn collect_items(source: &str, tokens: &[Token]) -> Vec<Item> {
    let index = LineIndex::new(source);
    let mut items = Vec::new();
    let mut cursor = 0usize;
    for (position, token) in tokens.iter().enumerate() {
        let start = token.pos;
        let end = token_end(source, token);
        scan_comments(source, cursor, start, &index, &mut items);
        if matches!(token.kind, Tok::Eof) {
            break;
        }
        let text = source[start..end].to_string();
        let generic_args_follow = tokens
            .get(position + 1)
            .map(|_| is_generic_open(tokens, position + 1))
            .unwrap_or(false);
        let type_atoms_follow = type_atoms_follow(tokens, position, &token.kind);
        items.push(Item::Token(TokItem {
            kind: token.kind.clone(),
            text,
            line: index.line_of(start),
            end_line: index.line_of(end.saturating_sub(1)),
            generic_args_follow,
            type_atoms_follow,
        }));
        cursor = end;
    }
    items
}

/// The number of type atoms that follow a `list` or `table` type name.
fn type_atoms_follow(tokens: &[Token], position: usize, kind: &Tok) -> usize {
    let Tok::Ident(name) = kind else {
        return 0;
    };
    let next_is_ident = |offset: usize| {
        matches!(
            tokens.get(position + offset).map(|token| &token.kind),
            Some(Tok::Ident(_))
        )
    };
    match name.as_str() {
        "list" => usize::from(next_is_ident(1)),
        "table" => 2 * usize::from(next_is_ident(1) && next_is_ident(2)),
        _ => 0,
    }
}

/// True when the token at `position` opens a generic argument list.
fn is_generic_open(tokens: &[Token], position: usize) -> bool {
    matches!(
        tokens.get(position).map(|token| &token.kind),
        Some(Tok::Sym(symbol)) if symbol == "<"
    ) && scan_type_args(tokens, position).is_some()
}

/// Scans a generic argument list and returns the index after the closing `>`.
fn scan_type_args(tokens: &[Token], position: usize) -> Option<usize> {
    let mut cursor = position + 1;
    let mut arguments = 0;
    loop {
        match tokens.get(cursor).map(|token| &token.kind) {
            Some(Tok::Ident(_)) => {
                cursor += 1;
                arguments += 1;
                if let Some(Tok::Sym(symbol)) = tokens.get(cursor).map(|token| &token.kind) {
                    if symbol == "<" {
                        cursor = scan_type_args(tokens, cursor)?;
                    }
                }
            }
            Some(Tok::Sym(symbol)) if symbol == "," => cursor += 1,
            Some(Tok::Sym(symbol)) if symbol == ">" => {
                return if arguments == 0 {
                    None
                } else {
                    Some(cursor + 1)
                };
            }
            _ => return None,
        }
    }
}

/// The byte offset just after the given token.
fn token_end(source: &str, token: &Token) -> usize {
    match &token.kind {
        Tok::Ident(text) | Tok::Num(text) => token.pos + text.len(),
        Tok::Sym(symbol) => token.pos + symbol.len(),
        Tok::Str(_) => {
            let bytes = source.as_bytes();
            let quote = bytes[token.pos];
            let mut cursor = token.pos + 1;
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                cursor += 1;
                if byte == b'\\' {
                    cursor += 1;
                    continue;
                }
                if byte == quote {
                    break;
                }
            }
            cursor
        }
        Tok::Eof => source.len(),
    }
}

/// Collects the comments that live between two tokens.
fn scan_comments(source: &str, from: usize, to: usize, index: &LineIndex, items: &mut Vec<Item>) {
    let bytes = source.as_bytes();
    let mut cursor = from;
    while cursor < to {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'/' && cursor + 1 < to && bytes[cursor + 1] == b'/' {
            let start = cursor;
            while cursor < to && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            push_comment(source, CommentKind::Line, start, cursor, index, items);
        } else if bytes[cursor] == b'/' && cursor + 1 < to && bytes[cursor + 1] == b'*' {
            let start = cursor;
            cursor += 2;
            while cursor + 1 < to && !(bytes[cursor] == b'*' && bytes[cursor + 1] == b'/') {
                cursor += 1;
            }
            let end = if cursor + 1 < to { cursor + 2 } else { to };
            cursor = end;
            push_comment(source, CommentKind::Block, start, end, index, items);
        } else if bytes[cursor] == b'-' && cursor + 1 < to && bytes[cursor + 1] == b'-' {
            let start = cursor;
            cursor += 2;
            match long_bracket_open(bytes, cursor) {
                Some((body_start, level)) => {
                    let mut body = body_start;
                    let mut closed = None;
                    while body < to {
                        if bytes[body] == b']' {
                            let mut close = body + 1;
                            let mut close_level = 0;
                            while bytes.get(close) == Some(&b'=') {
                                close += 1;
                                close_level += 1;
                            }
                            if close_level == level && bytes.get(close) == Some(&b']') {
                                closed = Some(close + 1);
                                break;
                            }
                        }
                        body += 1;
                    }
                    let end = closed.unwrap_or(to);
                    cursor = end;
                    push_comment(source, CommentKind::Block, start, end, index, items);
                }
                None => {
                    while cursor < to && bytes[cursor] != b'\n' {
                        cursor += 1;
                    }
                    push_comment(source, CommentKind::Line, start, cursor, index, items);
                }
            }
        } else {
            cursor += 1;
        }
    }
}

/// Detects a Lua long bracket (`[[`, `[=[`, ...) and returns the body offset
/// together with the nesting level.
fn long_bracket_open(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut cursor = start + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'[') {
        Some((cursor + 1, cursor - start - 1))
    } else {
        None
    }
}

fn push_comment(
    source: &str,
    kind: CommentKind,
    start: usize,
    end: usize,
    index: &LineIndex,
    items: &mut Vec<Item>,
) {
    items.push(Item::Comment(Comment {
        kind,
        text: source[start..end].to_string(),
        line: index.line_of(start),
        end_line: index.line_of(end.saturating_sub(1)),
        own_line: is_own_line(source, start),
    }));
}

/// True when only whitespace precedes the given position on its line.
fn is_own_line(source: &str, position: usize) -> bool {
    let bytes = source.as_bytes();
    let mut cursor = position;
    while cursor > 0 {
        let byte = bytes[cursor - 1];
        if byte == b'\n' {
            return true;
        }
        if !byte.is_ascii_whitespace() {
            return false;
        }
        cursor -= 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatted(source: &str) -> String {
        format_source(source).expect("the source should format")
    }

    /// The kinds of every token of a source file, used to prove that the
    /// formatter never changes the program itself.
    fn token_kinds(source: &str) -> Vec<String> {
        lex(source)
            .expect("the source should lex")
            .into_iter()
            .filter_map(|token| match token.kind {
                Tok::Ident(name) => Some(format!("ident:{name}")),
                Tok::Num(value) => Some(format!("number:{value}")),
                Tok::Str(value) => Some(format!("string:{value}")),
                Tok::Sym(symbol) => Some(format!("symbol:{symbol}")),
                Tok::Eof => None,
            })
            .collect()
    }

    #[test]
    fn formats_a_simple_program() {
        let source = "int x=2;if(x>1){print(\"yes\");}";
        assert_eq!(
            formatted(source),
            "int x = 2;\nif (x > 1) {\n    print(\"yes\");\n}\n"
        );
    }

    #[test]
    fn formats_functions_and_calls() {
        let source = "fn add(int a,int b)->int{return a+b;} int x=add(2,3);";
        assert_eq!(
            formatted(source),
            "fn add(int a, int b) -> int {\n    return a + b;\n}\nint x = add(2, 3);\n"
        );
    }

    #[test]
    fn formats_a_class_with_methods() {
        let source = "class hero{str name=\"Ada\";int health=100;constructor(str name){self.name=name;}fn isAlive()->bool{return health>0;}}";
        assert_eq!(
            formatted(source),
            "class hero {\n    \
                 str name = \"Ada\";\n    \
                 int health = 100;\n    \
                 constructor(str name) {\n        \
                     self.name = name;\n    \
                 }\n    \
                 fn isAlive() -> bool {\n        \
                     return health > 0;\n    \
                 }\n}\n"
        );
    }

    #[test]
    fn formats_nested_blocks() {
        let source = "if (a) { if (b) { print(1); } else { print(2); } }";
        assert_eq!(
            formatted(source),
            "if (a) {\n    if (b) {\n        print(1);\n    } else {\n        print(2);\n    }\n}\n"
        );
    }

    #[test]
    fn formats_loops() {
        let source = "for(i,10){printf(\"{i}\");} while(true){break;}";
        assert_eq!(
            formatted(source),
            "for (i, 10) {\n    printf(\"{i}\");\n}\nwhile (true) {\n    break;\n}\n"
        );
    }

    #[test]
    fn keeps_string_literals_byte_for_byte() {
        let source = r#"str s = "a\tb\\c\"d";"#;
        assert_eq!(formatted(source), "str s = \"a\\tb\\\\c\\\"d\";\n");
    }

    #[test]
    fn preserves_comments() {
        let source = "// leading\nint x = 1; // trailing\n/* block\n   comment */\n-- lua comment\n--[[ lua long ]]\nint y = 2;\n";
        let output = formatted(source);
        assert!(output.contains("// leading"), "{output}");
        assert!(output.contains("int x = 1; // trailing"), "{output}");
        assert!(output.contains("/* block"), "{output}");
        assert!(output.contains("comment */"), "{output}");
        assert!(output.contains("-- lua comment"), "{output}");
        assert!(output.contains("--[[ lua long ]]"), "{output}");
    }

    #[test]
    fn keeps_blank_lines_between_statements() {
        let source = "int a = 1;\n\nint b = 2;\n";
        assert_eq!(formatted(source), "int a = 1;\n\nint b = 2;\n");
    }

    #[test]
    fn formats_table_and_list_literals() {
        let source = "dict player={name=\"Andy\",age=15};list int values=[4,8,15];";
        assert_eq!(
            formatted(source),
            "dict player = { name = \"Andy\", age = 15 };\nlist int values = [4, 8, 15];\n"
        );
    }

    #[test]
    fn formats_generics_without_spaces() {
        let source = "fn identity<T>(T value)->T{return value;} Box<int> box=new Box<int>(1);";
        assert_eq!(
            formatted(source),
            "fn identity<T>(T value) -> T {\n    return value;\n}\nBox<int> box = new Box<int>(1);\n"
        );
    }

    #[test]
    fn formats_structs_and_interfaces() {
        let source = "struct Point{float x;float y;} interface Named{fn name()->str;}";
        assert_eq!(
            formatted(source),
            "struct Point {\n    float x;\n    float y;\n}\ninterface Named {\n    fn name() -> str;\n}\n"
        );
    }

    #[test]
    fn keeps_the_token_stream_unchanged() {
        let source = r#"
            // a comment
            class hero {
                str name = "Ada";
                constructor(str name) { self.name = name; }
                fn greet() -> str { return "hi " + name; }
            }
            local class Hero = new hero("Ada");
            print(Hero.greet());
        "#;
        let output = formatted(source);
        assert_eq!(token_kinds(source), token_kinds(&output));
    }

    #[test]
    fn is_idempotent() {
        let source = r#"
            // a comment
            class hero {
                str name = "Ada";

                constructor(str name) {
                    self.name = name;
                }

                fn greet() -> str {
                    if (name != "") {
                        return "hi " + name;
                    } else {
                        return "hi";
                    }
                }
            }

            local class Hero = new hero("Ada");
            print(Hero.greet());
        "#;
        let once = formatted(source);
        let twice = formatted(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn reports_lexer_errors() {
        let error = format_source("int x = \"unterminated;").unwrap_err();
        assert_eq!(error, "unterminated string at line 1, column 9");
    }
}
