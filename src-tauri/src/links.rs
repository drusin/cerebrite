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
//
// Tags (issue 09, CONTEXT.md's "Tag" entry) are pure syntax sugar for a link
// to a page literally titled the tag text -- no separate identity, table, or
// index entry. `extract_links` therefore recognizes both tag surface forms
// alongside `[[Link]]` and folds them into the very same `LinkOccurrence`
// list, so they automatically flow into the backlinks table with zero
// additional plumbing downstream (index.rs never needs to know a given
// occurrence came from a tag rather than a bracket link):
//   - `#[[multi word tag]]` -- the bracketed form, for tag text containing
//     spaces or other characters that can't appear in the bare form. Same
//     inner shape as `[[...]]` (single line, no nested brackets).
//   - `#tagname` -- the bare form. The character class chosen here (and
//     mirrored by `HASH_TAG_PATTERN` in src/wiki-link-plugin.ts on the
//     frontend) is `\w` (Unicode letters/digits/underscore) plus `-` and
//     `/`, i.e. `[\w/-]+`. That's deliberately permissive enough to allow a
//     flat, slash-containing tag like `#project/foo` to be one page title
//     ("project/foo"), not a hierarchy (per the ticket's "no hierarchy"
//     requirement) -- while still stopping at whitespace or any character
//     outside that class: another `#`, closing punctuation (`.`, `,`, `)`,
//     `]`, `"`, ...), or end of line, since none of those are word-ish.

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

/// The bare-tag word-character class, shared by the standalone regex below
/// and documented at the top of this file: Unicode letters/digits/underscore
/// (`\w`) plus `-` and `/`.
const TAG_WORD_CLASS: &str = r"[\w/-]+";

/// A single combined regex scanning for all three link/tag surface forms in
/// one pass, so occurrences come out of `captures_iter` in true document
/// order regardless of which form produced them. Named capture groups
/// distinguish which alternative matched:
///   - `bracket`: `[[...]]` (ticket 05/07 -- may carry a `#heading` fragment)
///   - `hash_bracket`: `#[[...]]` (ticket 09's bracketed tag form)
///   - `hash_tag`: `#tagname` (ticket 09's bare tag form)
///
/// Alternation order matters: `hash_bracket` is listed before `hash_tag` so
/// `#[[Some Tag]]` is matched as one bracketed-tag occurrence rather than a
/// bare `#` followed by literal `[[Some Tag]]` text -- though in practice
/// `hash_tag`'s class excludes `[`, so it could never partially match there
/// anyway; the ordering is kept for clarity as the primary defense.
fn combined_link_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let pattern = format!(
            r"\[\[(?P<bracket>[^\[\]\n]+)\]\]|#\[\[(?P<hash_bracket>[^\[\]\n]+)\]\]|#(?P<hash_tag>{TAG_WORD_CLASS})"
        );
        Regex::new(&pattern).expect("valid combined link/tag regex")
    })
}

/// Scans `body` for `[[...]]`, `#[[...]]`, and `#tagname` occurrences,
/// returning one `LinkOccurrence` per match, in document order. Per
/// CONTEXT.md's "Tag" definition, a tag is pure sugar for a link to a page
/// titled the tag text -- so both tag forms resolve into a `LinkOccurrence`
/// exactly like a bracket link, just without a `#heading` fragment (tags
/// don't support heading-level targets).
pub fn extract_links(body: &str) -> Vec<LinkOccurrence> {
    let mut out = Vec::new();

    for capture in combined_link_regex().captures_iter(body) {
        let whole = capture.get(0).expect("capture group 0 always matches");

        if let Some(bracket) = capture.name("bracket") {
            let raw_title = bracket.as_str();

            // Split off a trailing `#heading` fragment (ticket 07 syntax),
            // if any: the part before the first `#` is the page-level
            // target, the part after it (if non-empty once trimmed) is the
            // target heading's slug.
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
        } else if let Some(hash_bracket) = capture.name("hash_bracket") {
            let target_text = hash_bracket.as_str().trim();
            if target_text.is_empty() {
                continue;
            }
            out.push(LinkOccurrence {
                normalized_target: normalize_title(target_text),
                heading_slug: None,
                snippet: build_snippet(body, whole.start(), whole.end()),
            });
        } else if let Some(hash_tag) = capture.name("hash_tag") {
            let target_text = hash_tag.as_str();
            out.push(LinkOccurrence {
                normalized_target: normalize_title(target_text),
                heading_slug: None,
                snippet: build_snippet(body, whole.start(), whole.end()),
            });
        }
    }

    out
}

/// Converts a persisted page's frontmatter `tags: [foo, bar]` list (issue
/// 09) into `LinkOccurrence`s, one per tag -- the page-level counterpart of
/// an inline `#tag`: each entry is sugar for a link to a page titled the tag
/// text, contributing to that target's backlinks exactly like a `#tag` in
/// the body would. Normalization matches every other link target
/// (`normalize_title`), so `tags: [Todo]` and an inline `#todo` elsewhere
/// resolve to the same target and compare equal.
pub fn frontmatter_tag_occurrences(tags: &[String]) -> Vec<LinkOccurrence> {
    tags.iter()
        .filter_map(|tag| {
            let trimmed = tag.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(LinkOccurrence {
                normalized_target: normalize_title(trimmed),
                heading_slug: None,
                snippet: format!("(frontmatter tag: {trimmed})"),
            })
        })
        .collect()
}

/// Combines a page's body-derived link/tag occurrences with its frontmatter
/// `tags:` list (issue 09), in that order -- the single entry point callers
/// (index.rs) use so every place a page's outbound links are computed feeds
/// both sources identically.
pub fn extract_all_links(body: &str, frontmatter_tags: &[String]) -> Vec<LinkOccurrence> {
    let mut out = extract_links(body);
    out.extend(frontmatter_tag_occurrences(frontmatter_tags));
    out
}

/// One `[[Page#fragment]]` occurrence's heading fragment, located precisely
/// enough in `body` to be rewritten in place (ticket 08's rebuild-time
/// cleanup pass): `fragment_range` is the exact byte range of the raw
/// fragment text itself (e.g. `Some Heading` in `[[Page#Some Heading]]`),
/// trimmed of surrounding whitespace but *not* slugified, so a rewrite can
/// replace exactly that text (whatever it originally was) with a new slug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingLinkSpan {
    pub normalized_target: String,
    pub heading_slug: String,
    pub fragment_range: std::ops::Range<usize>,
}

/// Scans `body` for `[[Page#fragment]]` occurrences that carry a non-empty
/// heading fragment, returning one `HeadingLinkSpan` per match. Plain
/// page-level links (no `#`, or an empty fragment) are skipped, same as
/// `extract_links`.
pub fn find_heading_link_spans(body: &str) -> Vec<HeadingLinkSpan> {
    let mut out = Vec::new();

    for capture in wiki_link_regex().captures_iter(body) {
        let group = capture.get(1).expect("capture group 1 always matches");
        let raw_title = group.as_str();
        let group_start = group.start();

        let Some(hash_rel) = raw_title.find('#') else {
            continue;
        };
        let target_text = raw_title[..hash_rel].trim();
        if target_text.is_empty() {
            continue;
        }

        let fragment_raw = &raw_title[hash_rel + 1..];
        let fragment_trimmed = fragment_raw.trim();
        if fragment_trimmed.is_empty() {
            continue;
        }

        let leading_ws = fragment_raw.len() - fragment_raw.trim_start().len();
        let frag_start = group_start + hash_rel + 1 + leading_ws;
        let frag_end = frag_start + fragment_trimmed.len();

        out.push(HeadingLinkSpan {
            normalized_target: normalize_title(target_text),
            heading_slug: slugify_heading(fragment_trimmed),
            fragment_range: frag_start..frag_end,
        });
    }

    out
}

/// Rewrites every occurrence in `body` that targets `old_title` (matched via
/// `normalize_title`, exactly like `extract_links`) so it targets
/// `new_title` instead -- the page-rename feature's fix-up pass for inbound
/// (and self-referential) links, run once per page whose body might mention
/// the renamed page. Returns the possibly-unchanged body and whether
/// anything was actually rewritten, so callers can skip a write when
/// nothing matched.
///
/// Per the page-rename design:
///   - `[[Old Title]]` / `[[Old Title#Heading]]`: only the pre-`#` title
///     span is replaced (mirroring `find_heading_link_spans`'s span
///     computation), so any heading fragment survives untouched.
///   - `#[[Old Title]]` (bracketed tag sugar): same title-span replacement.
///   - `#oldtag` (bare tag sugar): replaced with the literal new title if it
///     fits the bare-tag word class (`TAG_WORD_CLASS`, no spaces or other
///     punctuation), otherwise converted to bracket form (`#[[New Title]]`)
///     since a bare tag can't losslessly hold an arbitrary title.
pub fn rewrite_links_to_title(body: &str, old_title: &str, new_title: &str) -> (String, bool) {
    let normalized_old = normalize_title(old_title);
    let mut replacements: Vec<(std::ops::Range<usize>, String)> = Vec::new();

    for capture in combined_link_regex().captures_iter(body) {
        let whole = capture.get(0).expect("capture group 0 always matches");

        if let Some(bracket) = capture.name("bracket") {
            let raw_title = bracket.as_str();
            let group_start = bracket.start();
            let title_part = match raw_title.find('#') {
                Some(idx) => &raw_title[..idx],
                None => raw_title,
            };
            if normalize_title(title_part) != normalized_old {
                continue;
            }
            // Replace the *entire* pre-`#` span (not just its trimmed
            // interior) so stray padding whitespace from the old link text
            // (e.g. `[[  old title  ]]`) doesn't survive the rewrite.
            let start = group_start;
            let end = start + title_part.len();
            replacements.push((start..end, new_title.to_string()));
        } else if let Some(hash_bracket) = capture.name("hash_bracket") {
            let raw_title = hash_bracket.as_str();
            let group_start = hash_bracket.start();
            if normalize_title(raw_title) != normalized_old {
                continue;
            }
            replacements.push((group_start..group_start + raw_title.len(), new_title.to_string()));
        } else if let Some(hash_tag) = capture.name("hash_tag") {
            let target_text = hash_tag.as_str();
            if normalize_title(target_text) != normalized_old {
                continue;
            }
            let fits_bare_tag = !new_title.is_empty()
                && new_title.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/');
            let replacement_body = if fits_bare_tag {
                new_title.to_string()
            } else {
                format!("[[{new_title}]]")
            };
            replacements.push((whole.start()..whole.end(), format!("#{replacement_body}")));
        }
    }

    if replacements.is_empty() {
        return (body.to_string(), false);
    }

    let mut new_body = body.to_string();
    for (range, text) in replacements.into_iter().rev() {
        new_body.replace_range(range, &text);
    }
    (new_body, true)
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
///
/// `pub(crate)` so search.rs's naive trashed-page snippet fallback (ticket
/// 13) can reuse the exact same char-boundary-safe slicing rather than
/// duplicating it.
pub(crate) fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Rounds `idx` up to the nearest valid UTF-8 char boundary in `s`.
pub(crate) fn ceil_char_boundary(s: &str, mut idx: usize) -> usize {
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

    #[test]
    fn find_heading_link_spans_locates_the_raw_fragment_text() {
        let body = "See [[Some Page#A Heading]] for more.";
        let spans = find_heading_link_spans(body);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].normalized_target, "some page");
        assert_eq!(spans[0].heading_slug, "a-heading");
        assert_eq!(&body[spans[0].fragment_range.clone()], "A Heading");
    }

    #[test]
    fn find_heading_link_spans_skips_page_only_links() {
        let body = "See [[Some Page]] for more.";
        assert!(find_heading_link_spans(body).is_empty());
    }

    #[test]
    fn extracts_a_bare_hash_tag() {
        let body = "Filed under #todo for now.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "todo");
        assert_eq!(links[0].heading_slug, None);
    }

    #[test]
    fn bare_hash_tag_allows_slashes_and_is_flat_not_hierarchical() {
        // Per the ticket: `#project/foo` is one page literally titled
        // "project/foo", not a nested tag under "project".
        let body = "See #project/foo for details.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "project/foo");
    }

    #[test]
    fn bare_hash_tag_allows_hyphens_and_underscores() {
        let body = "Tagged #some-tag_name here.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "some-tag_name");
    }

    #[test]
    fn bare_hash_tag_stops_at_whitespace() {
        let body = "#alpha #beta";
        let links = extract_links(body);
        let targets: Vec<&str> = links.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["alpha", "beta"]);
    }

    #[test]
    fn bare_hash_tag_stops_at_another_hash() {
        let body = "#alpha#beta";
        let links = extract_links(body);
        let targets: Vec<&str> = links.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["alpha", "beta"]);
    }

    #[test]
    fn bare_hash_tag_stops_at_closing_punctuation() {
        let body = "Is this done (#done)? Also #wip, and #urgent.";
        let links = extract_links(body);
        let targets: Vec<&str> = links.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["done", "wip", "urgent"]);
    }

    #[test]
    fn extracts_a_bracketed_multi_word_tag() {
        let body = "See #[[multi word tag]] here.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "multi word tag");
        assert_eq!(links[0].heading_slug, None);
    }

    #[test]
    fn bracketed_tag_does_not_leave_a_stray_hash_or_double_count() {
        let body = "#[[Multi Word]] appears once.";
        let links = extract_links(body);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].normalized_target, "multi word");
    }

    #[test]
    fn tags_and_bracket_links_coexist_in_document_order() {
        let body = "See [[Alpha]] then #beta then #[[Gamma Delta]] then [[Epsilon]].";
        let links = extract_links(body);
        let targets: Vec<&str> = links.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["alpha", "beta", "gamma delta", "epsilon"]);
    }

    #[test]
    fn tag_and_bracket_link_normalize_to_the_same_target() {
        let inline_tag = extract_links("#Todo").remove(0).normalized_target;
        let bracket_link = extract_links("[[  todo  ]]").remove(0).normalized_target;
        assert_eq!(inline_tag, bracket_link);
    }

    #[test]
    fn frontmatter_tags_become_link_occurrences() {
        let tags = vec!["Foo".to_string(), "bar".to_string()];
        let occurrences = frontmatter_tag_occurrences(&tags);
        let targets: Vec<&str> = occurrences.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["foo", "bar"]);
        assert!(occurrences.iter().all(|l| l.heading_slug.is_none()));
    }

    #[test]
    fn frontmatter_tag_occurrence_normalizes_the_same_as_an_inline_tag() {
        let from_frontmatter = frontmatter_tag_occurrences(&["Todo".to_string()]).remove(0).normalized_target;
        let from_body = extract_links("#todo").remove(0).normalized_target;
        assert_eq!(from_frontmatter, from_body);
    }

    #[test]
    fn extract_all_links_combines_body_and_frontmatter_tags_in_order() {
        let body = "Body has [[Alpha]] and #beta.";
        let tags = vec!["gamma".to_string()];
        let occurrences = extract_all_links(body, &tags);
        let targets: Vec<&str> = occurrences.iter().map(|l| l.normalized_target.as_str()).collect();
        assert_eq!(targets, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn rewrite_links_to_title_replaces_a_plain_bracket_link() {
        let (body, changed) = rewrite_links_to_title("See [[Old Title]] here.", "Old Title", "New Title");
        assert!(changed);
        assert_eq!(body, "See [[New Title]] here.");
    }

    #[test]
    fn rewrite_links_to_title_preserves_a_heading_fragment() {
        let (body, changed) =
            rewrite_links_to_title("See [[Old Title#Some Heading]] here.", "Old Title", "New Title");
        assert!(changed);
        assert_eq!(body, "See [[New Title#Some Heading]] here.");
    }

    #[test]
    fn rewrite_links_to_title_matches_case_and_whitespace_insensitively() {
        let (body, changed) = rewrite_links_to_title("See [[  old   title  ]] here.", "Old Title", "New Title");
        assert!(changed);
        assert_eq!(body, "See [[New Title]] here.");
    }

    #[test]
    fn rewrite_links_to_title_ignores_unrelated_links() {
        let (body, changed) = rewrite_links_to_title("See [[Something Else]] here.", "Old Title", "New Title");
        assert!(!changed);
        assert_eq!(body, "See [[Something Else]] here.");
    }

    #[test]
    fn rewrite_links_to_title_updates_a_bracketed_tag() {
        let (body, changed) = rewrite_links_to_title("Filed under #[[Old Title]].", "Old Title", "New Title");
        assert!(changed);
        assert_eq!(body, "Filed under #[[New Title]].");
    }

    #[test]
    fn rewrite_links_to_title_updates_a_bare_tag_when_the_new_title_still_fits() {
        let (body, changed) = rewrite_links_to_title("Filed under #oldtag.", "oldtag", "newtag");
        assert!(changed);
        assert_eq!(body, "Filed under #newtag.");
    }

    #[test]
    fn rewrite_links_to_title_converts_a_bare_tag_to_bracket_form_when_the_new_title_has_a_space() {
        let (body, changed) = rewrite_links_to_title("Filed under #todo.", "todo", "to do");
        assert!(changed);
        assert_eq!(body, "Filed under #[[to do]].");
    }

    #[test]
    fn rewrite_links_to_title_handles_multiple_occurrences_without_byte_offset_drift() {
        // Sanity check that an earlier replacement (shorter or longer than
        // the original match) doesn't shift byte ranges computed for a
        // later match -- replacements are applied back-to-front.
        let (body, changed) = rewrite_links_to_title(
            "First [[Unrelated]] then [[Old Title]] then #[[Old Title]] too.",
            "Old Title",
            "New Longer Title",
        );
        assert!(changed);
        assert_eq!(
            body,
            "First [[Unrelated]] then [[New Longer Title]] then #[[New Longer Title]] too."
        );
    }

    #[test]
    fn find_heading_link_spans_replacement_round_trips() {
        let body = "Intro [[Page#old-slug]] and [[Other#Another Heading]] tail.";
        let spans = find_heading_link_spans(body);
        assert_eq!(spans.len(), 2);

        // Replace back-to-front so earlier byte ranges stay valid.
        let mut new_body = body.to_string();
        for span in spans.iter().rev() {
            new_body.replace_range(span.fragment_range.clone(), "new-slug");
        }
        assert_eq!(
            new_body,
            "Intro [[Page#new-slug]] and [[Other#new-slug]] tail."
        );
    }
}
