// Frontmatter parsing for persisted pages (ADR-0005): every page's stable id
// lives in YAML frontmatter. If a file is missing an id, we generate one and
// write it back to disk so the id survives future opens/renames.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_yaml::Value;

/// The parsed, id-guaranteed result of reading one page file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPage {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// Splits `content` into an optional raw YAML frontmatter block and the
/// remaining body. Frontmatter is delimited by a `---` line at the very top
/// of the file and a closing `---` line.
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let Some(rest) = content.strip_prefix("---\n") else {
        return (None, content);
    };

    if let Some(end_idx) = rest.find("\n---\n") {
        let yaml = &rest[..end_idx];
        let body = &rest[end_idx + "\n---\n".len()..];
        return (Some(yaml), body);
    }

    // Frontmatter closes right at end of file, with no trailing body.
    if let Some(stripped) = rest.strip_suffix("\n---\n") {
        return (Some(stripped), "");
    }
    if let Some(stripped) = rest.strip_suffix("\n---") {
        return (Some(stripped), "");
    }

    (None, content)
}

/// Parses the frontmatter/body of already-loaded file content, ensuring an
/// `id` is present. Returns the parsed page plus, if the id had to be
/// generated, the full new file content that should be written back.
fn parse_content(content: &str, default_title: &str) -> (ParsedPage, Option<String>) {
    let (yaml_block, body) = split_frontmatter(content);

    let mut mapping = match yaml_block {
        Some(yaml) => match serde_yaml::from_str::<Value>(yaml) {
            Ok(Value::Mapping(m)) => m,
            _ => serde_yaml::Mapping::new(),
        },
        None => serde_yaml::Mapping::new(),
    };

    let id_key = Value::String("id".to_string());
    let existing_id = mapping.get(&id_key).and_then(|v| v.as_str()).map(|s| s.to_string());

    let (id, needs_rewrite) = match existing_id {
        Some(id) if !id.is_empty() => (id, false),
        _ => {
            let generated = generate_id();
            mapping.insert(id_key, Value::String(generated.clone()));
            (generated, true)
        }
    };

    let title_key = Value::String("title".to_string());
    let title = mapping
        .get(&title_key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| default_title.to_string());

    let new_content = if needs_rewrite {
        let yaml_str =
            serde_yaml::to_string(&Value::Mapping(mapping)).unwrap_or_else(|_| String::new());
        Some(format!("---\n{yaml_str}---\n{body}"))
    } else {
        None
    };

    (
        ParsedPage {
            id,
            title,
            body: body.to_string(),
        },
        new_content,
    )
}

/// Rewrites `content`'s body while preserving its existing frontmatter block
/// (delimiters and raw YAML) exactly as found, byte-for-byte. This is the
/// "serialize" counterpart to `split_frontmatter`/`parse_content`: the
/// editor only ever produces a new body, never a new frontmatter block, so
/// splicing the old frontmatter text back in is what keeps fields like `id`
/// preserved exactly across a save (per ADR-0003 / issue 03). If `content`
/// has no frontmatter block at all, `new_body` becomes the entire file.
fn splice_body(content: &str, new_body: &str) -> String {
    let (yaml_block, _old_body) = split_frontmatter(content);
    match yaml_block {
        Some(yaml) => format!("---\n{yaml}\n---\n{new_body}"),
        None => new_body.to_string(),
    }
}

/// Reads `path`, replaces its body with `new_body` while preserving the
/// existing frontmatter block byte-for-byte, and writes the result back to
/// disk.
pub fn write_body(path: &Path, new_body: &str) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading page file {}", path.display()))?;
    let new_content = splice_body(&content, new_body);
    fs::write(path, new_content)
        .with_context(|| format!("writing updated body to {}", path.display()))?;
    Ok(())
}

/// Mints a fresh, stable page id. Shared by both "fill in a missing id on an
/// existing file" (`parse_and_ensure_id`) and "mint an id for a brand-new
/// page" (issue 04's explicit "new page" action).
pub fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Normalizes a page title for link resolution (issue 05 / ADR-0009): a
/// dynamic page has no file and no frontmatter id, so it's identified
/// purely by this normalized form of its title.
///
/// The rule, chosen to be concrete and simple rather than clever: trim
/// leading/trailing whitespace, collapse any internal run of whitespace to
/// a single space, and lowercase the result. `split_whitespace` already
/// gives us trim + collapse for free; lowercasing on top makes `[[Todo]]`,
/// `[[ todo ]]` and `[[TODO]]` all resolve to the same page.
///
/// This same normalization is used to case/whitespace-insensitively match a
/// link's title against existing persisted pages' titles (`resolve_page`)
/// -- an existing page's own title/casing is never rewritten by this, only
/// compared against.
pub fn normalize_title(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Converts a page title into a filesystem-safe slug: lowercased, with any
/// run of non-alphanumeric characters (spaces, punctuation, ...) collapsed
/// into a single hyphen, and no leading/trailing hyphen. Unicode letters are
/// lowercased and kept (via `char::is_alphanumeric`) rather than stripped.
///
/// A title that slugifies to nothing (empty, or punctuation/whitespace only)
/// falls back to `"untitled"` so callers always get a usable filename stem.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_was_hyphen = true; // seed true so we never emit a leading hyphen

    for ch in title.trim().chars() {
        if ch.is_alphanumeric() {
            slug.extend(ch.to_lowercase());
            prev_was_hyphen = false;
        } else if !prev_was_hyphen {
            slug.push('-');
            prev_was_hyphen = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

/// Builds the full file content for a brand-new, explicitly-created page
/// (issue 04): YAML frontmatter with just `id` and `title`, and an
/// intentionally empty body -- no auto-inserted `# Title` heading, per the
/// ticket and CONTEXT.md's clean-markdown rule (ADR-0003).
pub fn new_page_content(id: &str, title: &str) -> String {
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(Value::String("id".to_string()), Value::String(id.to_string()));
    mapping.insert(Value::String("title".to_string()), Value::String(title.to_string()));
    let yaml_str =
        serde_yaml::to_string(&Value::Mapping(mapping)).unwrap_or_else(|_| String::new());
    format!("---\n{yaml_str}---\n")
}

/// Reads `path`, parses its frontmatter, generating and persisting a stable
/// id if one is missing, and returns the resulting page (id, title, body).
pub fn parse_and_ensure_id(path: &Path) -> Result<ParsedPage> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading page file {}", path.display()))?;

    let default_title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let (parsed, new_content) = parse_content(&content, &default_title);

    if let Some(new_content) = new_content {
        fs::write(path, new_content)
            .with_context(|| format!("writing generated id back to {}", path.display()))?;
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn parses_existing_id_and_title_without_rewriting_file() {
        let dir = tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "hello.md",
            "---\nid: abc-123\ntitle: Hello World\n---\nBody text.\n",
        );

        let parsed = parse_and_ensure_id(&path).unwrap();

        assert_eq!(parsed.id, "abc-123");
        assert_eq!(parsed.title, "Hello World");
        assert_eq!(parsed.body, "Body text.\n");

        // File must be untouched since the id was already present.
        let content_after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            content_after,
            "---\nid: abc-123\ntitle: Hello World\n---\nBody text.\n"
        );
    }

    #[test]
    fn generates_and_persists_missing_id() {
        let dir = tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "no-id.md",
            "---\ntitle: Has A Title\n---\nSome body.\n",
        );

        let parsed = parse_and_ensure_id(&path).unwrap();

        assert!(!parsed.id.is_empty());
        assert_eq!(parsed.title, "Has A Title");
        assert_eq!(parsed.body, "Some body.\n");

        // File on disk should now contain the generated id.
        let content_after = fs::read_to_string(&path).unwrap();
        assert!(content_after.contains(&format!("id: {}", parsed.id)));
        assert!(content_after.contains("title: Has A Title"));
        assert!(content_after.ends_with("Some body.\n"));

        // Re-parsing must return the same id (stable across re-reads).
        let parsed_again = parse_and_ensure_id(&path).unwrap();
        assert_eq!(parsed_again.id, parsed.id);
    }

    #[test]
    fn defaults_title_to_filename_stem_when_missing() {
        let dir = tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "My Page Name.md",
            "---\nid: xyz\n---\nBody only.\n",
        );

        let parsed = parse_and_ensure_id(&path).unwrap();

        assert_eq!(parsed.id, "xyz");
        assert_eq!(parsed.title, "My Page Name");
    }

    #[test]
    fn write_body_preserves_frontmatter_including_extra_fields() {
        let dir = tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "hello.md",
            "---\nid: abc-123\ntitle: Hello World\ntags:\n  - foo\n  - bar\n---\nOld body.\n",
        );

        write_body(&path, "New body with *different* content.\n").unwrap();

        let content_after = fs::read_to_string(&path).unwrap();
        assert_eq!(
            content_after,
            "---\nid: abc-123\ntitle: Hello World\ntags:\n  - foo\n  - bar\n---\nNew body with *different* content.\n"
        );

        // Round-trip: re-parsing must still see the same id/title, and tags
        // must remain in the raw frontmatter untouched.
        let parsed = parse_and_ensure_id(&path).unwrap();
        assert_eq!(parsed.id, "abc-123");
        assert_eq!(parsed.title, "Hello World");
        assert_eq!(parsed.body, "New body with *different* content.\n");
        assert!(content_after.contains("tags:\n  - foo\n  - bar"));
    }

    #[test]
    fn write_body_round_trip_parse_mutate_serialize_reparse() {
        let dir = tempdir().unwrap();
        let path = write_file(
            dir.path(),
            "page.md",
            "---\nid: keep-me\ntitle: Keep Title\ntags: [a, b]\n---\nOriginal body text.\n",
        );

        let before = parse_and_ensure_id(&path).unwrap();
        write_body(&path, "Edited body text.\n").unwrap();
        let after = parse_and_ensure_id(&path).unwrap();

        assert_eq!(before.id, after.id);
        assert_eq!(before.title, after.title);
        assert_eq!(after.body, "Edited body text.\n");

        let content_after = fs::read_to_string(&path).unwrap();
        assert!(content_after.contains("tags: [a, b]"));
    }

    #[test]
    fn write_body_on_file_with_no_frontmatter_writes_body_as_is() {
        let dir = tempdir().unwrap();
        let path = write_file(dir.path(), "plain.md", "Original plain content.\n");

        write_body(&path, "Replaced plain content.\n").unwrap();

        let content_after = fs::read_to_string(&path).unwrap();
        assert_eq!(content_after, "Replaced plain content.\n");
    }

    #[test]
    fn slugify_lowercases_and_hyphenates_spaces_and_punctuation() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  Trim Me  "), "trim-me");
        assert_eq!(slugify("Multiple   Spaces--and--dashes"), "multiple-spaces-and-dashes");
    }

    #[test]
    fn slugify_handles_unicode_letters() {
        assert_eq!(slugify("Café Déjà Vu"), "café-déjà-vu");
        assert_eq!(slugify("Über Cool"), "über-cool");
    }

    #[test]
    fn slugify_falls_back_to_untitled_for_punctuation_only_or_empty_titles() {
        assert_eq!(slugify("!!!???"), "untitled");
        assert_eq!(slugify(""), "untitled");
        assert_eq!(slugify("   "), "untitled");
    }

    #[test]
    fn new_page_content_has_frontmatter_only_and_empty_body_no_heading() {
        let content = new_page_content("abc-123", "My New Page");

        assert_eq!(content, "---\nid: abc-123\ntitle: My New Page\n---\n");
        assert!(!content.contains('#'));

        // Round-trips through the normal parser: id/title recovered, body empty.
        let dir = tempdir().unwrap();
        let path = write_file(dir.path(), "my-new-page.md", &content);
        let parsed = parse_and_ensure_id(&path).unwrap();
        assert_eq!(parsed.id, "abc-123");
        assert_eq!(parsed.title, "My New Page");
        assert_eq!(parsed.body, "");
    }

    #[test]
    fn normalize_title_trims_and_collapses_whitespace() {
        assert_eq!(normalize_title("  Some   Page  "), "some page");
        assert_eq!(normalize_title("Some\tPage\n"), "some page");
    }

    #[test]
    fn normalize_title_is_case_insensitive() {
        assert_eq!(normalize_title("Todo"), "todo");
        assert_eq!(normalize_title("TODO"), normalize_title("todo"));
        assert_eq!(normalize_title("  ToDo  "), normalize_title("todo"));
    }

    #[test]
    fn normalize_title_of_empty_or_whitespace_only_is_empty() {
        assert_eq!(normalize_title(""), "");
        assert_eq!(normalize_title("   "), "");
    }

    #[test]
    fn handles_file_with_no_frontmatter_at_all() {
        let dir = tempdir().unwrap();
        let path = write_file(dir.path(), "plain.md", "Just some content, no frontmatter.\n");

        let parsed = parse_and_ensure_id(&path).unwrap();

        assert!(!parsed.id.is_empty());
        assert_eq!(parsed.title, "plain");
        assert_eq!(parsed.body, "Just some content, no frontmatter.\n");

        let content_after = fs::read_to_string(&path).unwrap();
        assert!(content_after.starts_with("---\n"));
        assert!(content_after.contains(&format!("id: {}", parsed.id)));
        assert!(content_after.trim_end().ends_with("Just some content, no frontmatter."));
    }
}
