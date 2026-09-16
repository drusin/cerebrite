// Heading-level linking (issue 07 / ADR-0005): headings are addressed by a
// slug derived from their text, rather than a second frontmatter-owned id
// (which would violate the clean-markdown rule, ADR-0003, by putting
// tool-owned metadata into the body).
//
// The base slugification reuses `frontmatter::slugify` verbatim -- the same
// scheme already used to turn a page *title* into a filename slug (lowercase,
// unicode-aware alphanumeric runs kept, everything else collapsed to a single
// hyphen, no leading/trailing hyphen, "untitled" fallback for an
// all-punctuation/empty input). ADR-0005 doesn't prescribe a different scheme
// for headings, and reusing the existing one keeps exactly one slugging rule
// in the codebase instead of two subtly different ones.
//
// What *is* heading-specific is uniqueness scope: a page title's slug must be
// unique across the whole vault (it becomes a filename), but a heading's slug
// only needs to be unique *within its own page* (ADR-0005: "addressed by
// their slugified heading text ... within the page"). `HeadingSlugger` tracks
// that per-page uniqueness, appending a GitHub-style `-2`, `-3`, ... suffix to
// every slug after the first occurrence of a given base slug.

use std::collections::HashMap;

use crate::frontmatter::slugify;

/// Slugifies a single heading's text using the same base scheme as
/// `frontmatter::slugify` (page-title-to-filename slugging). This alone does
/// not dedupe against other headings on the same page -- see
/// `HeadingSlugger` for that.
pub fn slugify_heading(text: &str) -> String {
    slugify(text)
}

/// Assigns slugs to a page's headings in document order, appending `-2`,
/// `-3`, ... suffixes to every occurrence of a base slug after the first
/// (GitHub's heading-anchor scheme), so the ids stay unique within the page.
#[derive(Debug, Default)]
pub struct HeadingSlugger {
    seen: HashMap<String, usize>,
}

impl HeadingSlugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the slug for the next heading with this `text`, recording it
    /// so a later call with the same base slug gets the next suffix.
    pub fn slug(&mut self, text: &str) -> String {
        let base = slugify_heading(text);
        let count = self.seen.entry(base.clone()).or_insert(0);
        *count += 1;
        if *count == 1 {
            base
        } else {
            format!("{base}-{count}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_heading_basic_text() {
        assert_eq!(slugify_heading("Setup"), "setup");
        assert_eq!(slugify_heading("Getting Started"), "getting-started");
    }

    #[test]
    fn slugify_heading_strips_punctuation() {
        assert_eq!(slugify_heading("What's Next?"), "what-s-next");
        assert_eq!(slugify_heading("Step 1: Install"), "step-1-install");
    }

    #[test]
    fn slugify_heading_handles_unicode() {
        assert_eq!(slugify_heading("Über Setup"), "über-setup");
        assert_eq!(slugify_heading("Café Déjà Vu"), "café-déjà-vu");
    }

    #[test]
    fn slugger_leaves_the_first_occurrence_unsuffixed() {
        let mut slugger = HeadingSlugger::new();
        assert_eq!(slugger.slug("Setup"), "setup");
    }

    #[test]
    fn slugger_suffixes_repeated_headings_within_a_page() {
        let mut slugger = HeadingSlugger::new();
        assert_eq!(slugger.slug("Setup"), "setup");
        assert_eq!(slugger.slug("Setup"), "setup-2");
        assert_eq!(slugger.slug("Setup"), "setup-3");
    }

    #[test]
    fn slugger_tracks_distinct_base_slugs_independently() {
        let mut slugger = HeadingSlugger::new();
        assert_eq!(slugger.slug("Setup"), "setup");
        assert_eq!(slugger.slug("Usage"), "usage");
        assert_eq!(slugger.slug("Setup"), "setup-2");
        assert_eq!(slugger.slug("Usage"), "usage-2");
    }
}
