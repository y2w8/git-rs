use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Write as FmtWrite,
    io::{ErrorKind, Write as IoWrite},
    path::Path,
    sync::LazyLock,
};

use comrak::adapters::SyntaxHighlighterAdapter;
use tree_sitter_grammar_repository::{Grammar, Language};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

thread_local! {
    static HIGHLIGHTER: RefCell<Highlighter> = RefCell::new(Highlighter::new());
}

macro_rules! count {
    () => (0);
    ($e:expr) => (1);
    ($e:expr, $($rest:expr),*) => (1 + count!($($rest),*));
}

macro_rules! define_classes {
    ($($name:literal => $class:literal),*,) => {
        static HIGHLIGHT_NAMES: [&str; count!($($name),*)] = [
            $($name),*
        ];

        static HIGHLIGHT_CLASSES: [&str; count!($($name),*)] = [
            $($class),*
        ];
    };
}

define_classes! {
"attribute" => "attribute",
"boolean" => "boolean",
"carriage-return" => "carriage-return",
"comment" => "comment",
"comment.documentation" => "comment documentation",
"constant" => "constant",
"constant.builtin" => "constant builtin",
"constructor" => "constructor",
"constructor.builtin" => "constructor builtin",
"embedded" => "embedded",
"error" => "error",
"escape" => "escape",
"function" => "function",
"function.builtin" => "function builtin",
"keyword" => "keyword",
"markup" => "markup",
"markup.bold" => "markup bold",
"markup.heading" => "markup heading",
"markup.italic" => "markup italic",
"markup.link" => "markup link",
"markup.link.url" => "markup link url",
"markup.list" => "markup list",
"markup.list.checked" => "markup list checked",
"markup.list.numbered" => "markup list numbered",
"markup.list.unchecked" => "markup list unchecked",
"markup.list.unnumbered" => "markup list unnumbered",
"markup.quote" => "markup quote",
"markup.raw" => "markup raw",
"markup.raw.block" => "markup raw block",
"markup.raw.inline" => "markup raw inline",
"markup.strikethrough" => "markup strikethrough",
"module" => "module",
"number" => "number",
"operator" => "operator",
"property" => "property",
"property.builtin" => "property builtin",
"punctuation" => "punctuation",
"punctuation.bracket" => "punctuation bracket",
"punctuation.delimiter" => "punctuation delimiter",
"punctuation.special" => "punctuation special",
"string" => "string",
"string.escape" => "string escape",
"string.regexp" => "string regexp",
"string.special" => "string special",
"string.special.symbol" => "string special symbol",
"tag" => "tag",
"type" => "type",
"type.builtin" => "type builtin",
"variable" => "variable",
"variable.builtin" => "variable builtin",
"variable.member" => "variable member",
"variable.parameter" => "variable parameter",}

pub fn prime_highlighters() {
    let _res = HIGHLIGHTER_CONFIGS.len();
}

static HIGHLIGHTER_CONFIGS: LazyLock<Vec<HighlightConfiguration>> = LazyLock::new(|| {
    Grammar::VARIANTS
        .iter()
        .copied()
        .map(Grammar::highlight_configuration_params)
        .map(|v| {
            let mut configuration = HighlightConfiguration::new(
                v.language.into(),
                v.name,
                v.highlights_query,
                v.injection_query,
                v.locals_query,
            )
            .unwrap_or_else(|e| panic!("bad query for {}: {e}", v.name));
            configuration.configure(&HIGHLIGHT_NAMES);
            configuration
        })
        .collect()
});

pub fn fetch_highlighter_config(file: &Path) -> Option<&'static HighlightConfiguration> {
    Language::from_file_name(file)
        .map(Language::grammar)
        .map(Grammar::idx)
        .map(|idx| &HIGHLIGHTER_CONFIGS[idx])
}

pub fn fetch_highlighter_config_by_token(token: &str) -> Option<&'static HighlightConfiguration> {
    Language::from_injection(token)
        .map(Language::grammar)
        .map(Grammar::idx)
        .map(|idx| &HIGHLIGHTER_CONFIGS[idx])
}

pub struct ComrakHighlightAdapter;

impl SyntaxHighlighterAdapter for ComrakHighlightAdapter {
    fn write_highlighted(
        &self,
        output: &mut dyn IoWrite,
        lang: Option<&str>,
        code: &str,
    ) -> std::io::Result<()> {
        let out = format_file(code, FileIdentifier::Token(lang.unwrap_or_default()))
            .map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
        output.write_all(out.as_bytes())
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn IoWrite,
        _attributes: HashMap<String, String>,
    ) -> std::io::Result<()> {
        write!(output, "<pre>")
    }

    fn write_code_tag(
        &self,
        _output: &mut dyn IoWrite,
        _attributes: HashMap<String, String>,
    ) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum FileIdentifier<'a> {
    Path(&'a Path),
    Token(&'a str),
}

fn render_line_prefix(tag: bool, line_number: usize) -> String {
    if !tag {
        return "".to_string();
    }
    format!("<span class='lnb'><a id='n{0}' href='#n{0}'>{0}</a></span><code>", line_number)
}

fn render_line_suffix(tag: bool, line_number: usize) -> String {
    if !tag {
        return "\n".to_string();
    }
    "</code>\n".to_string()
}

pub fn format_file(content: &str, identifier: FileIdentifier<'_>) -> Result<String. Error> {
    let mut out = String::new();
    format_file_inner(&mut out, content, identifier, true)?;
    Ok(out)
}

pub fn format_file_inner(
    out: &mut String,
    content: &str,
    identifier: FileIdentifier<'_>,
    code_tag: bool,
) -> anyhow::Result<()> {
    let config = match identifier {
        FileIdentifier::Path(v) => fetch_highlighter_config(v),
        FileIdentifier::Token(v) => fetch_highlighter_config_by_token(v),
    };

    let Some(config) = config else {
        for (i, line) in content.lines().enumerate() {
            out.push_str(&render_line_prefix(code_tag, i));
            v_htmlescape::b_escape(line.as_bytes(), out);
            out.push_str(&render_line_suffix(code_tag, i));
        }

        return Ok(());
    };

    let mut line_number = 0;
    HIGHLIGHTER.with_borrow_mut(|highlighter| {
        highlighter.parser().reset();

        let spans = highlighter.highlight(config, content.as_bytes(), None, |injection| {
            debug!(injection, "Highlighter switch requested");
            fetch_highlighter_config_by_token(injection)
        });

        let mut spans = match spans {
            Ok(v) => v,
            Err(error) => {
                error!(
                    ?error,
                    "Failed to run highlighter, falling back to plaintext"
                );

                for line in content.lines() {
                    line_number = line_number + 1;
                    out.push_str(&render_line_prefix(code_tag, line_number));
                    v_htmlescape::b_escape(line.as_bytes(), out);
                    out.push_str(&render_line_suffix(code_tag, line_number));
                }

                return Ok(());
            }
        };

        let mut tag_open = true;
        line_number = line_number + 1;
        out.push_str(&render_line_prefix(code_tag, line_number));

        while let Some(span) = spans.next().transpose()? {
            if !tag_open {
                line_number = line_number + 1;
                out.push_str(&render_line_prefix(code_tag, line_number));
                tag_open = true;
            }

            match span {
                HighlightEvent::Source { start, end } => {
                    let content = &content[start..end];

                    for (i, line) in content.lines().enumerate() {
                        if i != 0 {
                            out.push_str(&render_line_suffix(code_tag, line_number));
                            line_number = line_number + 1;
                            out.push_str(&render_line_prefix(code_tag, line_number));
                        }
                        v_htmlescape::b_escape(line.as_bytes(), out);
                    }

                    if content.ends_with('\n') {
                        out.push_str(&render_line_suffix(code_tag, line_number));
                        tag_open = false;
                    }
                }
                HighlightEvent::HighlightStart(highlight) => {
                    write!(
                        out,
                        r#"<span class="highlight {}">"#,
                        HIGHLIGHT_CLASSES[highlight.0]
                    )?;
                }
                HighlightEvent::HighlightEnd => {
                    out.push_str("</span>");
                }
            }
        }

        if tag_open {
            out.push_str(&render_line_suffix(code_tag, line_number));
        }

        Ok::<_, anyhow::Error>(())
    })?;

    Ok(())
}
