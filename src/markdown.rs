use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};

use crate::syntax_highlight;

pub fn render(raw: &str, repo_name: &str, ref_name: &str) -> String {
    let events: Vec<Event> = Parser::new_ext(raw, Options::all()).collect();
    let events = link_headings(events);
    let events = highlight_code_blocks(events);
    let events = transform_media_and_links(events, repo_name, ref_name);

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    rewrite_raw_html_img_srcs(&out, repo_name, ref_name)
}

fn transform_media_and_links<'a>(
    events: Vec<Event<'a>>,
    repo_name: &str,
    ref_name: &str,
) -> Vec<Event<'a>> {
    let mut output = Vec::with_capacity(events.len());
    let mut i = 0;

    while i < events.len() {
        match &events[i] {
            // 1. Markdown image `![alt](url)`
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let resolved_url = resolve_media_url(dest_url, repo_name, ref_name);

                // Collect alt text inside Start(Image) .. End(Image)
                let mut j = i + 1;
                let mut alt = String::new();
                while j < events.len() {
                    match &events[j] {
                        Event::End(TagEnd::Image) => break,
                        Event::Text(t) | Event::Code(t) => alt.push_str(t),
                        _ => {}
                    }
                    j += 1;
                }

                if is_video_ext(&resolved_url) {
                    output.push(Event::Html(CowStr::from(video_html(&resolved_url, false))));
                    i = (j + 1).min(events.len());
                } else if is_github_asset_url(&resolved_url) {
                    // GitHub user-attachments URLs have no file extension; render <img> that falls back to <video> if it's a video
                    output.push(Event::Html(CowStr::from(github_asset_img_html(
                        &resolved_url,
                        &alt,
                    ))));
                    i = (j + 1).min(events.len());
                } else {
                    output.push(Event::Start(Tag::Image {
                        link_type: *link_type,
                        dest_url: CowStr::from(resolved_url),
                        title: title.clone(),
                        id: id.clone(),
                    }));
                    i += 1;
                }
            }

            // 2. Markdown link `[text](url)` or `<url>` pointing to a video or GitHub asset
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let url_str = dest_url.as_ref();
                if is_video_ext(url_str) || is_github_asset_url(url_str) {
                    let resolved_url = resolve_media_url(url_str, repo_name, ref_name);
                    let mut j = i + 1;
                    while j < events.len() && !matches!(events[j], Event::End(TagEnd::Link)) {
                        j += 1;
                    }
                    output.push(Event::Html(CowStr::from(video_html(
                        &resolved_url,
                        is_github_asset_url(&resolved_url),
                    ))));
                    i = (j + 1).min(events.len());
                } else {
                    let resolved_url = resolve_link_url(url_str, repo_name, ref_name);
                    output.push(Event::Start(Tag::Link {
                        link_type: *link_type,
                        dest_url: CowStr::from(resolved_url),
                        title: title.clone(),
                        id: id.clone(),
                    }));
                    i += 1;
                }
            }

            // 3. Bare URL on its own in text (how GitHub embeds drag-and-dropped videos in READMEs)
            Event::Text(text) => {
                let trimmed = text.trim();
                if (trimmed.starts_with("https://") || trimmed.starts_with("http://"))
                    && !trimmed.contains(char::is_whitespace)
                    && (is_github_asset_url(trimmed) || is_video_ext(trimmed))
                {
                    output.push(Event::Html(CowStr::from(video_html(
                        trimmed,
                        is_github_asset_url(trimmed),
                    ))));
                } else {
                    output.push(events[i].clone());
                }
                i += 1;
            }

            _ => {
                output.push(events[i].clone());
                i += 1;
            }
        }
    }

    output
}

fn is_relative_url(url: &str) -> bool {
    !url.is_empty()
        && !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("//")
        && !url.starts_with('/')
        && !url.starts_with('#')
        && !url.starts_with("mailto:")
        && !url.starts_with("data:")
}

/// Resolves relative image/video paths (e.g. `docs/screenshots/tree.png`) to `/<repo>/raw/<path>?h=<ref>`.
fn resolve_media_url(url: &str, repo_name: &str, ref_name: &str) -> String {
    if is_relative_url(url) {
        let clean = url.trim_start_matches("./").trim_start_matches('/');
        format!("/{repo_name}/raw/{clean}?h={ref_name}")
    } else {
        url.to_string()
    }
}

/// Resolves relative repository links (e.g. `docs/screenshots/` or `LICENSE`) to `/<repo>/tree/<path>?h=<ref>`.
fn resolve_link_url(url: &str, repo_name: &str, ref_name: &str) -> String {
    if is_relative_url(url) {
        let clean = url.trim_start_matches("./").trim_start_matches('/');
        format!("/{repo_name}/tree/{clean}?h={ref_name}")
    } else {
        url.to_string()
    }
}

fn is_video_ext(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url).to_lowercase();
    path.ends_with(".mp4")
        || path.ends_with(".webm")
        || path.ends_with(".mov")
        || path.ends_with(".ogv")
        || path.ends_with(".m4v")
}

/// Matches GitHub uploaded asset URLs:
/// - `https://github.com/user-attachments/assets/<uuid>`
/// - `https://github.com/<owner>/<repo>/assets/<user_id>/<uuid>`
/// - `https://private-user-images.githubusercontent.com/...`
fn is_github_asset_url(url: &str) -> bool {
    url.starts_with("https://github.com/user-attachments/assets/")
        || (url.starts_with("https://github.com/") && url.contains("/assets/"))
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn video_html(url: &str, fallback_to_img: bool) -> String {
    let url = escape_attr(url);
    if fallback_to_img {
        format!(
            r#"<video class="readme-video" controls playsinline preload="metadata" src="{url}" onerror="const img=document.createElement('img');img.src=this.src;img.className='readme-img';this.replaceWith(img);"></video>"#
        )
    } else {
        format!(
            r#"<video class="readme-video" controls playsinline preload="metadata" src="{url}"></video>"#
        )
    }
}

fn github_asset_img_html(url: &str, alt: &str) -> String {
    let url = escape_attr(url);
    let alt = escape_attr(alt);
    format!(
        r#"<img class="readme-img" src="{url}" alt="{alt}" onerror="const v=document.createElement('video');v.src=this.src;v.controls=true;v.playsInline=true;v.preload='metadata';v.className='readme-video';this.replaceWith(v);">"#
    )
}

/// Also rewrites relative `<img src="...">` and `<video src="...">` inside raw HTML blocks in Markdown.
fn rewrite_raw_html_img_srcs(html: &str, repo_name: &str, ref_name: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(idx) = rest.find("src=\"") {
        let (before, after_prefix) = rest.split_at(idx + 5);
        result.push_str(before);

        if let Some(end_quote) = after_prefix.find('"') {
            let src_val = &after_prefix[..end_quote];
            let resolved = resolve_media_url(src_val, repo_name, ref_name);
            result.push_str(&resolved);
            rest = &after_prefix[end_quote..];
        } else {
            rest = after_prefix;
            break;
        }
    }
    result.push_str(rest);
    result
}

fn highlight_code_blocks(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut output = Vec::with_capacity(events.len());
    let mut code_lang: Option<String> = None;
    let mut code_buf = String::new();

    for event in events {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        lang.split_whitespace().next().unwrap_or("").to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                code_lang = Some(lang);
                code_buf.clear();
            }
            Event::Text(text) if code_lang.is_some() => {
                code_buf.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(lang) = code_lang.take() {
                    let highlighted = syntax_highlight::highlight_block(&code_buf, &lang);
                    output.push(Event::Html(CowStr::from(highlighted)));
                }
            }
            other => output.push(other),
        }
    }

    output
}

fn link_headings(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut output = Vec::with_capacity(events.len());
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut i = 0;

    while i < events.len() {
        if let Event::Start(Tag::Heading {
            level,
            classes,
            attrs,
            id: None,
        }) = &events[i]
        {
            let level = *level;
            let classes = classes.clone();
            let attrs = attrs.clone();

            let mut j = i + 1;
            let mut text = String::new();
            while j < events.len() {
                match &events[j] {
                    Event::End(TagEnd::Heading(_)) => break,
                    Event::Text(t) | Event::Code(t) => text.push_str(t),
                    _ => {}
                }
                j += 1;
            }

            let mut slug = slugify(&text);
            if slug.is_empty() {
                slug = "section".to_string();
            }
            let count = seen.entry(slug.clone()).or_insert(0);
            if *count > 0 {
                slug = format!("{slug}-{count}");
            }
            *count += 1;

            output.push(Event::Start(Tag::Heading {
                level,
                id: Some(CowStr::from(slug.clone())),
                classes,
                attrs,
            }));
            output.push(Event::Start(Tag::Link {
                link_type: LinkType::Inline,
                dest_url: CowStr::from(format!("#{slug}")),
                title: CowStr::from(""),
                id: CowStr::from(""),
            }));

            output.extend_from_slice(&events[i + 1..j]);
            output.push(Event::End(TagEnd::Link));
            output.push(events[j].clone()); // End(Heading)
            i = j + 1;
        } else {
            output.push(events[i].clone());
            i += 1;
        }
    }

    output
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for c in text.to_lowercase().chars() {
        if c.is_alphanumeric() {
            slug.push(c);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}
