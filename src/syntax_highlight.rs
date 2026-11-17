use std::path::Path;

const IMPORTS: &[&str] = &[
    "use", "mod", "extern", "import", "from", "export", "package", "require", "include",
];

const KEYWORDS: &[&str] = &[
    "fn",
    "let",
    "mut",
    "pub",
    "priv",
    "struct",
    "enum",
    "impl",
    "trait",
    "type",
    "where",
    "for",
    "in",
    "while",
    "loop",
    "if",
    "else",
    "match",
    "return",
    "break",
    "continue",
    "async",
    "await",
    "move",
    "ref",
    "static",
    "const",
    "unsafe",
    "dyn",
    "as",
    "local",
    "function",
    "then",
    "elseif",
    "end",
    "do",
    "repeat",
    "until",
    "and",
    "or",
    "not",
    "def",
    "class",
    "try",
    "except",
    "finally",
    "with",
    "yield",
    "lambda",
    "pass",
    "raise",
    "switch",
    "case",
    "default",
    "var",
    "interface",
    "go",
    "defer",
    "select",
    "chan",
    "fallthrough",
    "elif",
    "fi",
    "done",
    "esac",
];

const BOOLEANS: &[&str] = &["true", "false", "nil", "null", "undefined"];

const BUILTIN_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32",
    "f64", "bool", "str", "char", "int", "float", "double", "void", "auto", "string", "number",
    "boolean", "any",
];

const BUILTIN_CONSTS: &[&str] = &["Some", "None", "Ok", "Err"];

const BUILTIN_FUNCS: &[&str] = &[
    "drop", "print", "ipairs", "pairs", "pcall", "xpcall", "tonumber", "tostring", "assert",
    "error", "len", "range", "echo", "set",
];

const BUILTIN_VARS: &[&str] = &["self", "Self", "super", "crate", "this"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Code {
        hash_comments: bool,
        dash_comments: bool,
        rust_like: bool,
    },
    Config,
    Style,
    Markup,
    Plain,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    BlockComment,
    LuaBlockComment,
    DoubleString,
    RawString(usize),
}

pub fn highlight_lines(source: &str, path_or_lang: &str) -> Vec<String> {
    let mode = detect_mode(path_or_lang, source);
    let mut state = State::Normal;

    source
        .lines()
        .map(|line| highlight_line(line, mode, &mut state))
        .collect()
}

pub fn highlight_block(source: &str, lang: &str) -> String {
    let lines = highlight_lines(source, lang);
    format!("<pre><code>{}</code></pre>", lines.join("\n"))
}

fn detect_mode(path_or_lang: &str, source: &str) -> Mode {
    let lower = path_or_lang.trim().to_ascii_lowercase();
    let file_name = Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&lower);
    let ext = Path::new(file_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);

    if matches!(
        file_name,
        "justfile"
            | "makefile"
            | "gnumakefile"
            | "dockerfile"
            | "containerfile"
            | "pkgbuild"
            | ".bashrc"
            | ".zshrc"
            | ".profile"
    ) {
        return Mode::Code {
            hash_comments: true,
            dash_comments: false,
            rust_like: false,
        };
    }

    if matches!(
        file_name,
        ".gitignore" | ".gitmodules" | ".gitattributes" | ".editorconfig" | "cargo.lock"
    ) {
        return Mode::Config;
    }

    match ext {
        "rs" | "rust" | "ron" => Mode::Code {
            hash_comments: false,
            dash_comments: false,
            rust_like: true,
        },
        "lua" | "sql" => Mode::Code {
            hash_comments: false,
            dash_comments: true,
            rust_like: false,
        },
        "sh" | "bash" | "zsh" | "fish" | "py" | "python" | "rb" | "ruby" | "nix" | "pl" | "r"
        | "just" | "mk" | "cmake" => Mode::Code {
            hash_comments: true,
            dash_comments: false,
            rust_like: false,
        },
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "go" | "golang" | "js" | "jsx" | "ts"
        | "tsx" | "javascript" | "typescript" | "java" | "cs" | "csharp" | "zig" | "kt"
        | "kotlin" | "swift" | "dart" | "php" | "glsl" | "wgsl" | "hlsl" => Mode::Code {
            hash_comments: false,
            dash_comments: false,
            rust_like: false,
        },
        "toml" | "ini" | "cfg" | "conf" | "yaml" | "yml" | "json" | "jsonc" | "kdl" | "service"
        | "timer" | "socket" | "target" | "mount" | "desktop" | "env" => Mode::Config,
        "css" | "scss" | "sass" | "less" => Mode::Style,
        "html" | "htm" | "xml" | "svg" | "xhtml" => Mode::Markup,
        _ if source.starts_with("#!") => Mode::Code {
            hash_comments: true,
            dash_comments: false,
            rust_like: false,
        },
        _ => Mode::Plain,
    }
}

fn highlight_line(line: &str, mode: Mode, state: &mut State) -> String {
    match mode {
        Mode::Plain => escape_html(line),
        Mode::Config => highlight_config_line(line),
        Mode::Style => highlight_style_line(line, state),
        Mode::Markup => highlight_markup_line(line, state),
        Mode::Code {
            hash_comments,
            dash_comments,
            rust_like,
        } => highlight_code_line(line, hash_comments, dash_comments, rust_like, state),
    }
}

fn highlight_code_line(
    line: &str,
    hash_comments: bool,
    dash_comments: bool,
    rust_like: bool,
    state: &mut State,
) -> String {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut out = String::with_capacity(len * 2);
    let mut prev_keyword = "";
    let mut prev_was_dot = false;
    let mut in_fn_params = false;
    let mut paren_depth: usize = 0;

    while i < len {
        match *state {
            State::BlockComment => {
                if let Some(pos) = line[i..].find("*/") {
                    let end = i + pos + 2;
                    push_span(&mut out, "hl-comment", &line[i..end]);
                    i = end;
                    *state = State::Normal;
                } else {
                    push_span(&mut out, "hl-comment", &line[i..]);
                    return out;
                }
                continue;
            }
            State::LuaBlockComment => {
                if let Some(pos) = line[i..].find("]]") {
                    let end = i + pos + 2;
                    push_span(&mut out, "hl-comment", &line[i..end]);
                    i = end;
                    *state = State::Normal;
                } else {
                    push_span(&mut out, "hl-comment", &line[i..]);
                    return out;
                }
                continue;
            }
            State::DoubleString => {
                i = scan_double_string(line, i, &mut out, state);
                continue;
            }
            State::RawString(hashes) => {
                i = scan_raw_string(line, i, hashes, &mut out, state);
                continue;
            }
            State::Normal => {}
        }

        let b = bytes[i];

        if b.is_ascii_whitespace() {
            let start = i;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            out.push_str(&line[start..i]);
            continue;
        }

        // `//` and `/*` comments
        if b == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                push_span(&mut out, "hl-comment", &line[i..]);
                break;
            }
            if bytes[i + 1] == b'*' {
                *state = State::BlockComment;
                continue;
            }
        }

        // Lua `--` and `--[[` comments
        if dash_comments && b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            if line[i + 2..].starts_with("[[") {
                *state = State::LuaBlockComment;
                continue;
            }
            push_span(&mut out, "hl-comment", &line[i..]);
            break;
        }

        // `#` attributes, comments, or preprocessor directives
        if b == b'#' {
            if rust_like
                && i + 1 < len
                && (bytes[i + 1] == b'[' || bytes[i + 1] == b'!')
                && let Some(close) = line[i..].find(']')
            {
                let end = i + close + 1;
                push_span(&mut out, "hl-attribute", &line[i..end]);
                i = end;
                continue;
            }
            if hash_comments || (i == 0 && line.starts_with("#!")) {
                push_span(&mut out, "hl-comment", &line[i..]);
                break;
            }
            if !rust_like {
                let start = i;
                i += 1;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                while i < len && bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                push_span(&mut out, "hl-macro", &line[start..i]);
                continue;
            }
        }

        // Rust raw strings `r"..."`, `r#"..."#`
        if rust_like && b == b'r' && i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') {
            let mut hashes = 0;
            let mut j = i + 1;
            while j < len && bytes[j] == b'#' {
                hashes += 1;
                j += 1;
            }
            if j < len && bytes[j] == b'"' {
                push_span(&mut out, "hl-string", &line[i..=j]);
                *state = State::RawString(hashes);
                i = j + 1;
                continue;
            }
        }

        if b == b'"' {
            push_span(&mut out, "hl-string", "\"");
            *state = State::DoubleString;
            i = scan_double_string(line, i + 1, &mut out, state);
            prev_was_dot = false;
            continue;
        }

        if b == b'\'' {
            if rust_like {
                if let Some((end, is_lifetime)) = scan_rust_quote(line, i) {
                    let class = if is_lifetime { "hl-label" } else { "hl-escape" };
                    push_span(&mut out, class, &line[i..end]);
                    i = end;
                    prev_was_dot = false;
                    continue;
                }
            } else {
                let end = scan_single_string(line, i);
                push_span(&mut out, "hl-string", &line[i..end]);
                i = end;
                prev_was_dot = false;
                continue;
            }
        }

        if b.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()))
            {
                i += 1;
            }
            push_span(&mut out, "hl-number", &line[start..i]);
            prev_was_dot = false;
            continue;
        }

        if is_ident_start(b) {
            let start = i;
            i += 1;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let ident = &line[start..i];
            let rest = &line[i..];

            if rest.starts_with('!') && !rest.starts_with("!=") {
                i += 1;
                push_span(&mut out, "hl-macro", &line[start..i]);
                prev_was_dot = false;
                continue;
            }

            let class = classify_ident(
                ident,
                rest.trim_start(),
                prev_keyword,
                prev_was_dot,
                in_fn_params && paren_depth == 1,
            );
            push_span(&mut out, class, ident);

            if matches!(ident, "fn" | "function" | "def") {
                in_fn_params = true;
            }
            prev_keyword = if matches!(class, "hl-keyword" | "hl-include") {
                ident
            } else {
                ""
            };
            prev_was_dot = false;
            continue;
        }

        if matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';') {
            match b {
                b'(' => paren_depth += 1,
                b')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                    if paren_depth == 0 {
                        in_fn_params = false;
                    }
                }
                b'{' | b';' => in_fn_params = false,
                _ => {}
            }
            push_span(&mut out, "hl-punctuation", &line[i..=i]);
            prev_was_dot = false;
            i += 1;
            continue;
        }

        if b == b':' {
            let len_colon = if i + 1 < len && bytes[i + 1] == b':' {
                2
            } else {
                1
            };
            push_span(&mut out, "hl-punctuation", &line[i..i + len_colon]);
            prev_was_dot = false;
            i += len_colon;
            continue;
        }

        if b == b'.' {
            if i + 1 < len && bytes[i + 1] == b'.' {
                let end = if i + 2 < len && bytes[i + 2] == b'=' {
                    i + 3
                } else {
                    i + 2
                };
                push_span(&mut out, "hl-operator", &line[i..end]);
                prev_was_dot = false;
                i = end;
            } else {
                push_span(&mut out, "hl-punctuation", ".");
                prev_was_dot = true;
                i += 1;
            }
            continue;
        }

        if is_operator_byte(b) {
            let start = i;
            i += 1;
            while i < len && is_operator_byte(bytes[i]) {
                i += 1;
            }
            push_span(&mut out, "hl-operator", &line[start..i]);
            prev_was_dot = false;
            continue;
        }

        let ch = line[i..].chars().next().unwrap();
        let end = i + ch.len_utf8();
        escape_into(&mut out, &line[i..end]);
        prev_was_dot = false;
        i = end;
    }

    out
}

fn classify_ident(
    ident: &str,
    trimmed_rest: &str,
    prev_keyword: &str,
    prev_was_dot: bool,
    in_fn_params: bool,
) -> &'static str {
    if !prev_was_dot {
        if IMPORTS.contains(&ident) {
            return "hl-include";
        }
        if KEYWORDS.contains(&ident) {
            return "hl-keyword";
        }
        if BOOLEANS.contains(&ident) {
            return "hl-boolean";
        }
        if BUILTIN_TYPES.contains(&ident) {
            return "hl-type-builtin";
        }
        if BUILTIN_CONSTS.contains(&ident) {
            return "hl-constant";
        }
        if BUILTIN_FUNCS.contains(&ident) {
            return "hl-function-builtin";
        }
        if BUILTIN_VARS.contains(&ident) {
            return "hl-variable";
        }
    }

    if matches!(prev_keyword, "fn" | "function" | "def") {
        return "hl-function";
    }
    if matches!(
        prev_keyword,
        "struct" | "enum" | "trait" | "type" | "class" | "interface"
    ) {
        return "hl-type";
    }
    if prev_keyword == "mod" {
        return "hl-module";
    }

    if ident.as_bytes()[0].is_ascii_uppercase() {
        let is_const = ident.len() > 1
            && ident
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
        return if is_const { "hl-constant" } else { "hl-type" };
    }

    if trimmed_rest.starts_with("::") && !trimmed_rest.starts_with("::<") {
        return "hl-module";
    }
    if trimmed_rest.starts_with('(') || trimmed_rest.starts_with("::<") {
        return "hl-function";
    }
    if prev_was_dot {
        return "hl-property";
    }
    if trimmed_rest.starts_with(':') && !trimmed_rest.starts_with("::") {
        return if in_fn_params {
            "hl-parameter"
        } else {
            "hl-property"
        };
    }

    "hl-variable"
}

fn highlight_config_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with(';') {
        let mut out = String::new();
        push_span(&mut out, "hl-comment", line);
        return out;
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let indent = &line[..line.len() - trimmed.len()];
        let mut out = String::from(indent);
        let open_len = if trimmed.starts_with("[[") { 2 } else { 1 };
        let close_len = if trimmed.ends_with("]]") { 2 } else { 1 };
        push_span(&mut out, "hl-punctuation", &trimmed[..open_len]);
        push_span(
            &mut out,
            "hl-property",
            &trimmed[open_len..trimmed.len() - close_len],
        );
        push_span(
            &mut out,
            "hl-punctuation",
            &trimmed[trimmed.len() - close_len..],
        );
        return out;
    }

    if let Some(sep_idx) = find_config_sep(line) {
        let key_part = &line[..sep_idx];
        let sep = &line[sep_idx..=sep_idx];
        let val_part = &line[sep_idx + 1..];

        let mut out = String::new();
        let key_trim = key_part.trim();
        if !key_trim.is_empty() {
            let lead = key_part.len() - key_part.trim_start().len();
            let trail = key_part.len() - key_part.trim_end().len();
            out.push_str(&key_part[..lead]);
            push_span(&mut out, "hl-property", key_trim);
            out.push_str(&key_part[key_part.len() - trail..]);
        } else {
            escape_into(&mut out, key_part);
        }

        let sep_class = if sep == "=" {
            "hl-operator"
        } else {
            "hl-punctuation"
        };
        push_span(&mut out, sep_class, sep);

        let mut state = State::Normal;
        out.push_str(&highlight_code_line(
            val_part, true, false, false, &mut state,
        ));
        return out;
    }

    let mut state = State::Normal;
    highlight_code_line(line, true, false, false, &mut state)
}

fn find_config_sep(line: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (i, &b) in line.as_bytes().iter().enumerate() {
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && (b == b'=' || b == b':') {
            return Some(i);
        }
    }
    None
}

fn highlight_style_line(line: &str, state: &mut State) -> String {
    let trimmed = line.trim_start();
    if *state == State::BlockComment || trimmed.starts_with("/*") {
        return highlight_code_line(line, false, false, false, state);
    }

    if let Some(colon) = line.find(':')
        && !trimmed.starts_with(['.', '#', '@'])
        && !line[..colon].contains('{')
    {
        let prop = &line[..colon];
        let rest = &line[colon + 1..];
        let mut out = String::new();
        let lead = prop.len() - prop.trim_start().len();
        out.push_str(&prop[..lead]);
        push_span(&mut out, "hl-property", prop.trim_start());
        push_span(&mut out, "hl-punctuation", ":");
        out.push_str(&highlight_code_line(rest, false, false, false, state));
        return out;
    }

    highlight_code_line(line, false, false, false, state)
}

fn highlight_markup_line(line: &str, state: &mut State) -> String {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut out = String::with_capacity(len * 2);

    while i < len {
        if *state == State::BlockComment {
            if let Some(pos) = line[i..].find("-->") {
                let end = i + pos + 3;
                push_span(&mut out, "hl-comment", &line[i..end]);
                i = end;
                *state = State::Normal;
            } else {
                push_span(&mut out, "hl-comment", &line[i..]);
                return out;
            }
            continue;
        }

        if line[i..].starts_with("<!--") {
            *state = State::BlockComment;
            continue;
        }

        if bytes[i] == b'<' {
            let start = i;
            i += 1;
            if i < len && matches!(bytes[i], b'/' | b'!' | b'?') {
                i += 1;
            }
            push_span(&mut out, "hl-tag-delimiter", &line[start..i]);

            let tag_start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b':')) {
                i += 1;
            }
            if i > tag_start {
                push_span(&mut out, "hl-tag", &line[tag_start..i]);
            }

            while i < len && bytes[i] != b'>' {
                if bytes[i].is_ascii_whitespace() {
                    out.push(bytes[i] as char);
                    i += 1;
                } else if matches!(bytes[i], b'"' | b'\'') {
                    let quote = bytes[i];
                    let s_start = i;
                    i += 1;
                    while i < len && bytes[i] != quote {
                        i += 1;
                    }
                    if i < len {
                        i += 1;
                    }
                    push_span(&mut out, "hl-string", &line[s_start..i]);
                } else if bytes[i] == b'=' {
                    push_span(&mut out, "hl-operator", "=");
                    i += 1;
                } else if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'>' {
                    push_span(&mut out, "hl-tag-delimiter", "/>");
                    i += 2;
                    break;
                } else {
                    let attr_start = i;
                    while i < len
                        && !bytes[i].is_ascii_whitespace()
                        && !matches!(bytes[i], b'=' | b'>' | b'/' | b'"' | b'\'')
                    {
                        i += 1;
                    }
                    if i > attr_start {
                        push_span(&mut out, "hl-label", &line[attr_start..i]);
                    } else {
                        escape_into(&mut out, &line[i..=i]);
                        i += 1;
                    }
                }
            }

            if i < len && bytes[i] == b'>' {
                push_span(&mut out, "hl-tag-delimiter", ">");
                i += 1;
            }
            continue;
        }

        let next_lt = line[i..].find('<').map(|p| i + p).unwrap_or(len);
        escape_into(&mut out, &line[i..next_lt]);
        i = next_lt;
    }

    out
}

fn scan_double_string(line: &str, mut i: usize, out: &mut String, state: &mut State) -> usize {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut seg_start = i;

    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            if i > seg_start {
                push_span(out, "hl-string", &line[seg_start..i]);
            }
            let esc_end = (i + 2).min(len);
            push_span(out, "hl-escape", &line[i..esc_end]);
            i = esc_end;
            seg_start = i;
        } else if bytes[i] == b'"' {
            push_span(out, "hl-string", &line[seg_start..=i]);
            *state = State::Normal;
            return i + 1;
        } else {
            i += line[i..].chars().next().unwrap().len_utf8();
        }
    }

    if seg_start < len {
        push_span(out, "hl-string", &line[seg_start..len]);
    }
    len
}

fn scan_raw_string(
    line: &str,
    mut i: usize,
    hashes: usize,
    out: &mut String,
    state: &mut State,
) -> usize {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let start = i;

    while i < len {
        if bytes[i] == b'"' {
            let mut h = 0;
            while i + 1 + h < len && bytes[i + 1 + h] == b'#' && h < hashes {
                h += 1;
            }
            if h == hashes {
                let end = i + 1 + hashes;
                push_span(out, "hl-string", &line[start..end]);
                *state = State::Normal;
                return end;
            }
        }
        i += 1;
    }

    if start < len {
        push_span(out, "hl-string", &line[start..len]);
    }
    len
}

fn scan_single_string(line: &str, start: usize) -> usize {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = start + 1;
    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            i += 2;
        } else if bytes[i] == b'\'' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    len
}

fn scan_rust_quote(line: &str, start: usize) -> Option<(usize, bool)> {
    let rest = &line[start + 1..];
    if let Some(escaped) = rest.strip_prefix('\\')
        && let Some(close) = escaped.find('\'')
    {
        return Some((start + 2 + close + 1, false));
    }
    let mut chars = rest.char_indices();
    if let Some((_, first)) = chars.next() {
        if let Some((next_idx, '\'')) = chars.next() {
            return Some((start + 1 + next_idx + 1, false));
        }
        if first.is_ascii_alphabetic() || first == '_' {
            let mut end = first.len_utf8();
            for (idx, ch) in rest[end..].char_indices() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    end = first.len_utf8() + idx + ch.len_utf8();
                } else {
                    break;
                }
            }
            return Some((start + 1 + end, true));
        }
    }
    None
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_operator_byte(b: u8) -> bool {
    matches!(
        b,
        b'=' | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'&'
            | b'|'
            | b'^'
            | b'!'
            | b'<'
            | b'>'
            | b'?'
            | b'~'
            | b'@'
    )
}

fn push_span(out: &mut String, class: &str, text: &str) {
    if text.is_empty() {
        return;
    }
    out.push_str("<span class=\"");
    out.push_str(class);
    out.push_str("\">");
    escape_into(out, text);
    out.push_str("</span>");
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_into(&mut out, text);
    out
}

fn escape_into(out: &mut String, text: &str) {
    v_htmlescape::b_escape(text.as_bytes(), out);
}
