// Read-only markdown rendering. Done in Rust (rather than shipping a JS
// markdown lib) so ticket 03's WYSIWYG editor can reuse the same parser.

use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::heading_slug::HeadingSlugger;

pub fn render(body: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(body, options);
    let events: Vec<Event> = parser.collect();
    let events = add_heading_ids(events);

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    html_output
}

/// Concatenates the plain text of the heading whose `Start(Tag::Heading)`
/// event sits at `start_idx`, by scanning forward to its matching
/// `End(TagEnd::Heading)` -- the same text a reader would see, used as the
/// input to `HeadingSlugger` so a rendered heading's `id` matches exactly
/// what a `[[Page#Heading]]` link's slug resolves to (ticket 07 / ADR-0005).
fn heading_text(events: &[Event], start_idx: usize) -> String {
    let mut text = String::new();
    let mut depth = 0usize;

    for event in &events[start_idx + 1..] {
        match event {
            Event::Start(Tag::Heading { .. }) => depth += 1,
            Event::End(TagEnd::Heading(_)) => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Event::Text(t) | Event::Code(t) => text.push_str(t),
            _ => {}
        }
    }

    text
}

/// Rewrites every heading's `Start` event to carry a stable `id` (a slug of
/// its text, deduped per-page via `HeadingSlugger`), so `pulldown_cmark`'s
/// HTML renderer emits `<h2 id="the-slug">` -- an addressable anchor a
/// `[[Page#Heading]]` link's click-through can scroll straight to.
fn add_heading_ids<'a>(events: Vec<Event<'a>>) -> Vec<Event<'a>> {
    let mut slugger = HeadingSlugger::new();
    let mut out = Vec::with_capacity(events.len());

    for (idx, event) in events.iter().enumerate() {
        match event {
            Event::Start(Tag::Heading {
                level,
                id: _,
                classes,
                attrs,
            }) => {
                let text = heading_text(&events, idx);
                let slug = slugger.slug(&text);
                out.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(CowStr::from(slug)),
                    classes: classes.clone(),
                    attrs: attrs.clone(),
                }));
            }
            other => out.push(other.clone()),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown_to_html() {
        let html = render("# Title\n\nSome *body* text.\n");
        assert!(html.contains("<em>body</em>"));
    }

    #[test]
    fn headings_get_a_slugified_id() {
        let html = render("## Getting Started\n\nBody.\n");
        assert!(
            html.contains(r#"<h2 id="getting-started">Getting Started</h2>"#),
            "unexpected html: {html}"
        );
    }

    #[test]
    fn duplicate_headings_get_deduped_ids() {
        let html = render("# Setup\n\nOne.\n\n# Setup\n\nTwo.\n");
        assert!(html.contains(r#"<h1 id="setup">Setup</h1>"#), "unexpected html: {html}");
        assert!(html.contains(r#"<h1 id="setup-2">Setup</h1>"#), "unexpected html: {html}");
    }

    #[test]
    fn heading_ids_strip_punctuation_and_keep_unicode() {
        let html = render("# What's Café Setup?\n");
        assert!(html.contains(r#"id="what-s-café-setup"#), "unexpected html: {html}");
    }
}
