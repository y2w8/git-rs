use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, html};

pub fn render(raw: &str) -> String {
    let events: Vec<Event> = Parser::new_ext(raw, Options::all()).collect();
    let events = link_headings(events);

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
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
