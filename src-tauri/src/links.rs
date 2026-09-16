// Extracts `[[Link]]` occurrences out of a page's raw markdown body (issue
// 06). Rust has no access to the frontend's remark/mdast pipeline
// (wiki-link-plugin.ts), so this ports the same surface syntax
// (`\[\[...\]\]`, single line, no nested brackets -- see that file's
// `WIKI_LINK_PATTERN`) to a plain regex scan. This is the pragmatic choice
// documented on the ticket: a full micromark/mdast reimplementation in Rust
// just to exclude code spans would be a lot of surface area for a feature
// that only needs "does this page's body reference that title."
//
// Two edge cases the ticket calls out explicitly:
//   - A trailing `#heading` fragment (`[[Page#Heading]]`, ticket 07 syntax)
//     is split off: the text before the first `#` is the target page title,
//     and the text after it is slugified (`heading_slug::slugify_heading`,
//     ADR-0005) into the target heading's slug, carried alongside the page
//     target rather than discarded.
//   - Incidental double-bracket text: the pattern requires two literal `[`
//     immediately followed eventually by two literal `]` with nothing but a
//     single line of non-bracket text in between, so unmatched brackets
//     (`[[foo`), single-bracket text (`[foo]`), and bracket pairs split
//     across lines never match.

use regex::Regex;
use std::sync::OnceLock;

use crate::frontmatter::normalize_title;
use crate::heading_slug::slugify_heading;

/// One `[[...]]` occurrence found in a page's body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkOccurrence {
    /// The link's target title, normalized the same way `resolve_page` and
    /// `get_backlinks` normalize titles, so a source page's outbound link
    /// and a target page's own title compare equal regardless of casing or
    /// whitespace.
    pub normalized_target: String,
    /// The target heading's slug (ticket 07), if the link included a
    /// `#Heading` fragment -- `None` for a plain page-level link.
    pub heading_slug: Option<String>,
    /// ~80-100 characters of surrounding plain text from the source body,
    /// for display in the target page's Backlinks section.
    pub snippet: String,
}

/// How many characters of context to include on each side of the `[[...]]`
/// match when building a snippet (so a snippet is roughly
/// `2 * SNIPPET_RADIUS` characters, plus the link text itself).
const SNIPPET_RADIUS: usize = 40;

fn wiki_link_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[([^\[\]\n]+)\]\]").expect("valid wiki-link regex"))
}

/// Scans `body` for `[[...]]` occurrences, returning one `LinkOccurrence`
/// per match, in document order.
pub fn extract_links(body: &str) -> Vec<LinkOccurrence> {
    let mut out = Vec::new();

    for capture in wiki_link_regex().captures_iter(body) {
        let whole = capture.get(0).expect("capture group 0 always matches");
        let raw_title = capture.get(1).expect("capture group 1 always matches").as_str();

        // Split off a trailing `#heading` fragment (ticket 07 syntax), if
        // any: the part before the first `#` is the page-level target, the
        // part after it (if non-empty once trimmed) is the target heading's
        // slug.
        let mut parts = raw_title.splitn(2, '#');
        let target_text = parts.next().unwrap_or("").trim();
        let heading_slug = parts
            .next()
            .map(str::trim)
            .filter(|fragment| !fragment.is_empty())
            .map(slugify_heading);

        if target_text.is_empty() {
            continue;
        }

        out.push(LinkOccurrence {
            normalized_target: normalize_title(target_text),
            heading_slug,
            snippet: build_snippet(body, whole.start(), whole.end()),
        });
    }

    out
}

/// Builds a plain-text snippet of `body` around the byte range
/// `[match_start, match_end)`, expanded by `SNIPPET_RADIUS` characters on
/// each side, with whitespace/newlines collapsed to single spaces and an
/// ellipsis on whichever side(s) were truncated.
fn build_snippet(body: &str, match_start: usize, match_end: usize) -> String {
    let start = floor_char_boundary(body, match_start.saturating_sub(SNIPPET_RADIUS));
    let end = ceil_char_boundary(body, (match_end + SNIPPET_RADIUS).min(body.len()));

    let raw = &body[start..end];
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < body.len() { "…" } else { "" };
    format!("{prefix}{collapsed}{suffix}")
}

/// Rounds `idx` down to the nearest valid UTF-8 char boundary in `s` (the
/// stable-Rust equivalent of the still-nightly-only `str::floor_char_boundary`).
fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Rounds `idx` up to the nearest valid UTF-8 char boundary in `s`.
fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_single_link_with_snippet() {
        let body = "Some intro text. See [[Target Page]] for details.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "target page");
        assert_eq!(links[0].heading_slug, None);
        assert!(links[0].snippet.contains("[[Target Page]]"));
    }

    #[test]
    fn extracts_multiple_links_in_document_order() {
        let body = "First [[Alpha]] then later [[Beta]] and finally [[Gamma]].";
        let links = extract_links(body);
        let targets: Vec<&str> = links.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn captures_trailing_heading_fragment_as_a_slug() {
        let body = "See [[Some Page#A Heading]] for more.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "some page");
        assert_eq!(links[0].heading_slug.as_deref(), Some("a-heading"));
    }

    #[test]
    fn page_only_link_has_no_heading_slug() {
        let body = "See [[Some Page]] for more.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].heading_slug, None);
    }

    #[test]
    fn empty_heading_fragment_is_treated_as_a_page_only_link() {
        let body = "See [[Some Page#]] for more.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "some page");
        assert_eq!(links[0].heading_slug, None);
    }

    #[test]
    fn heading_only_fragment_with_no_title_is_ignored() {
        // `[[#Heading]]` has no page-level target at all -- nothing to record.
        let body = "See [[#Just A Heading]] here.";
        let links = extract_links(body);
        assert!(links.is_empty());
    }

    #[test]
    fn no_false_positives_from_unrelated_bracket_text() {
        let body = "Array access like foo[i][j], a single [bracket] pair, and an \
                    unterminated [[oops start of a link with no close.";
        let links = extract_links(body);
        assert!(links.is_empty(), "expected no matches, got {links:?}");
    }

    #[test]
    fn does_not_match_across_lines() {
        let body = "[[Unterminated on one line\nand closes]] on the next.";
        let links = extract_links(body);
        assert!(links.is_empty());
    }

    #[test]
    fn snippet_is_truncated_with_ellipses_on_a_long_body() {
        let filler = "word ".repeat(50);
        let body = format!("{filler}[[Middle Link]] {filler}");
        let links = extract_links(&body);
        assert_eq!(links.len(), 1);
        let snippet = &links[0].snippet;
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert!(snippet.contains("[[Middle Link]]"));
        // Roughly bounded: link text + 2*radius + a couple ellipsis chars,
        // generously padded since word-collapsing can shift exact lengths.
        assert!(snippet.chars().count() < 140);
    }

    #[test]
    fn normalizes_target_case_and_whitespace() {
        let body = "See [[  My   Page  ]] here.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "my page");
    }
}
