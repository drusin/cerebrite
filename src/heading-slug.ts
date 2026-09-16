// Heading-level linking (issue 07 / ADR-0005): mirrors
// src-tauri/src/heading_slug.rs's `slugify_heading` exactly (lowercase,
// unicode-aware letters/digits kept, every other run of characters
// collapsed to a single hyphen, no leading/trailing hyphen, "untitled"
// fallback for an all-punctuation/empty input) so a heading's id inside the
// Milkdown/ProseMirror editor DOM matches the slug a `[[Page#Heading]]` link
// resolves to on the Rust side.
//
// This is wired into the editor via Milkdown's own `headingIdGenerator`
// context slice (see page-editor.ts), which already handles per-page
// duplicate-heading dedup (its `syncHeadingIdPlugin`) -- so this function
// only needs to cover the base slugification, not the dedup counter itself.
// One known, accepted mismatch: Milkdown's built-in dedup suffixes a repeat
// as `-#2`/`-#3`, not the `-2`/`-3` scheme `HeadingSlugger` uses on the Rust
// side -- see page-editor.ts's comment for why this is left as-is.
export function slugifyHeadingText(text: string): string {
  let slug = "";
  let prevWasHyphen = true; // seed true so we never emit a leading hyphen

  for (const ch of text.trim()) {
    if (isAlphanumeric(ch)) {
      slug += ch.toLowerCase();
      prevWasHyphen = false;
    } else if (!prevWasHyphen) {
      slug += "-";
      prevWasHyphen = true;
    }
  }

  slug = slug.replace(/-+$/, "");

  return slug.length > 0 ? slug : "untitled";
}

function isAlphanumeric(ch: string): boolean {
  return /\p{L}|\p{N}/u.test(ch);
}

/// Turns a heading slug back into a human-readable label for display (e.g.
/// backlink entries' "→ Setup" annotation): splits on hyphens and
/// capitalizes each word. This is a best-effort, lossy reversal -- the
/// backlinks table only stores the slug, not the original heading text --
/// but it round-trips correctly for the common case of a slug produced from
/// plain title-case words.
export function humanizeHeadingSlug(slug: string): string {
  return slug
    .split("-")
    .filter((word) => word.length > 0)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}
