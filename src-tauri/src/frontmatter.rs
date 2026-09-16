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
            let generated = uuid::Uuid::new_v4().to_string();
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
