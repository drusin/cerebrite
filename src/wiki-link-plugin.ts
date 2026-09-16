// `[[Page Name]]` link syntax (issue 05): a small custom Milkdown plugin
// that recognizes `[[...]]` in the markdown source, renders it as a
// clickable inline "chip" node in the WYSIWYG view, and serializes back to
// `[[...]]` unchanged on save.
//
// Approach chosen (documented per the ticket, since this can't be verified
// visually): rather than writing a full micromark tokenizer for `[[...]]`
// (the more "complete" but much larger-surface-area option -- escaping,
// line-continuation, etc.), this piggybacks on a small, standard mdast
// mechanism:
//
//   - CommonMark's own bracket-matching leaves unmatched `[`/`]` characters
//     as literal text when they don't form a real link/reference (which
//     `[[Foo]]` never does, since nothing follows with `(url)` or
//     `[label]`), so a plain commonmark parse of `[[Foo]]` already produces
//     one ordinary text node containing the literal string `[[Foo]]`.
//   - `mdast-util-find-and-replace` (already a transitive dependency of
//     remark) then walks the already-parsed tree and splits any text node
//     containing `[[<title>]]` into a custom `wikiLink` mdast node carrying
//     the raw bracket contents verbatim (no trimming/escaping).
//   - A matching `toMarkdownExtensions` handler (the same `this.data(...)`
//     mechanism Milkdown's own preset plugins use, e.g.
//     remark-html-transformer) stringifies that node straight back to
//     `[[<value>]]`, byte for byte.
//
// Net effect: the round trip is exact (no added escaping -- satisfying
// ticket 03's clean-markdown guarantee), and the plugin is just a node
// schema + a find-and-replace transform + a to-markdown handler, all
// wired up the same way Milkdown's own commonmark preset wires up its
// node/mark schemas.
//
// `[[Page#Heading]]` (heading-level linking, ticket 07) is not special-cased
// here -- the chip's `title` attribute carries the raw bracket contents
// verbatim (e.g. "Page#Heading"), `#` included, exactly as typed. It's the
// Rust side (resolve_page, links.rs) and main.ts's navigation that split the
// page target from the heading fragment; this plugin only needs to keep the
// round trip exact, which it already does for any raw text a `[[...]]` can
// contain.
//
// Tags (issue 09, CONTEXT.md's "Tag" entry) are pure syntax sugar for the
// exact same link: `#tagname` and `#[[multi word tag]]` resolve through the
// identical dynamic/persisted page mechanism as `[[Page]]`, and per the
// ticket must render with "no distinct visual style" from an ordinary link
// chip. Rather than invent a second node type, both tag forms are parsed
// into the *same* `wikiLink` mdast node / `wiki_link` prosemirror node as a
// plain `[[Link]]` -- the only difference is a `form` field/attr
// ("bracket" | "hashBracket" | "hash") recording which surface syntax
// produced the chip, read only by the to-markdown serializer so a tag
// round-trips back to `#tag` / `#[[tag]]` (not silently rewritten into
// `[[tag]]`). The chip's DOM (`wiki-link-chip` class, `data-wiki-link-title`
// attribute) is identical either way, so the existing click handler in
// page-editor.ts and the existing visual style both apply to tags for free.
import { findAndReplace } from "mdast-util-find-and-replace";
import type { Root as MdastRoot } from "mdast";
import { $inputRule, $nodeSchema, $remark } from "@milkdown/kit/utils";
import { InputRule } from "@milkdown/kit/prose/inputrules";

/** The mdast node "type" used for a parsed `[[...]]` / `#tag` / `#[[...]]` run. */
const MDAST_TYPE = "wikiLink";

/** The ProseMirror node name registered in the schema. */
const NODE_NAME = "wiki_link";

/** Which surface syntax produced a chip, so the serializer can round-trip it exactly. */
type LinkForm = "bracket" | "hashBracket" | "hash";
const DEFAULT_FORM: LinkForm = "bracket";

/** Matches `[[<title>]]` where `<title>` is a single line with no nested brackets. */
const WIKI_LINK_PATTERN = /\[\[([^[\]\n]+)\]\]/g;

/**
 * Matches `#[[<tag>]]` (issue 09's bracketed tag syntax, for tags containing
 * spaces) -- same inner shape as `WIKI_LINK_PATTERN`, just with a leading
 * `#` that's swallowed along with the brackets (the chip shows only the tag
 * text, not the delimiters).
 */
const HASH_BRACKET_TAG_PATTERN = /#\[\[([^[\]\n]+)\]\]/g;

/**
 * Matches bare `#tagname` (issue 09's single-word tag syntax). The character
 * class is the exact rule chosen for "word-ish, flat-namespace" tag text:
 * Unicode letters/digits/underscore (via `\w`, which the `regex`/JS engines
 * both treat as Unicode-aware) plus `-` and `/` so a flat, slash-containing
 * tag like `#project/foo` is one page title ("project/foo"), not a
 * hierarchy. The run stops at whitespace or any character outside that
 * class -- e.g. another `#`, closing punctuation (`.`, `,`, `)`, `]`, `"`,
 * ...), or end of line -- since none of those are in the class. This must
 * stay in lockstep with `TAG_WORD_CHARS` in src-tauri/src/links.rs.
 */
const HASH_TAG_PATTERN = /#([\w/-]+)/g;

/**
 * Live-typing counterpart of `HASH_TAG_PATTERN`: since a bare tag has no
 * closing delimiter, the input rule instead fires on the first "boundary"
 * character typed after the tag's word-ish run (whitespace or a punctuation
 * mark that can't be part of a title), converting everything before it into
 * a chip and leaving the boundary character in the document as ordinary
 * text right after the chip.
 */
const HASH_TAG_INPUT_PATTERN = /#([\w/-]+)([ \t.,;:!?)\]}"'])$/;

/** Node types whose text content should never be treated as a wiki-link/tag source. */
const IGNORED_ANCESTOR_TYPES = ["code", "inlineCode"];

interface WikiLinkMdastNode {
  type: string;
  value?: string;
  form?: LinkForm;
}

/**
 * `mdast-util-to-markdown` extension: stringifies a `wikiLink` mdast node
 * back to its original surface syntax -- `[[<value>]]`, `#[[<value>]]`, or
 * `#<value>` depending on `form` -- with no escaping, so a value round-trips
 * exactly as typed regardless of what characters it contains.
 */
const wikiLinkToMarkdownExtension = {
  handlers: {
    [MDAST_TYPE]: (node: WikiLinkMdastNode) => {
      const value = node.value ?? "";
      switch (node.form) {
        case "hash":
          return `#${value}`;
        case "hashBracket":
          return `#[[${value}]]`;
        default:
          return `[[${value}]]`;
      }
    },
  },
};

/** Minimal shape of the unified `Processor` this attacher needs -- just enough to
 * read/write its plugin-shared `data` bag; the rest of the real `Processor` API
 * (parse/stringify/etc.) is irrelevant here. */
interface DataBag {
  data: (key: string, value?: unknown) => unknown;
}

/**
 * The remark plugin: registers the to-markdown handler above, and (as its
 * transform step, run once per parse via `remark.runSync`) splits any
 * `[[...]]` run found in text nodes into a `wikiLink` mdast node.
 *
 * Written as a `function` (not an arrow function) because unified calls it
 * with `this` bound to the processor -- that's how it registers the
 * to-markdown extension via `this.data(...)`, the same mechanism used by
 * `@milkdown/preset-commonmark`'s own remark plugins.
 */
function wikiLinkRemarkAttacher(this: DataBag) {
  const existing = (this.data("toMarkdownExtensions") as unknown[] | undefined) ?? [];
  this.data("toMarkdownExtensions", [...existing, wikiLinkToMarkdownExtension]);

  return (tree: MdastRoot) => {
    // Order matters: `#[[...]]` must be recognized *before* the plain
    // `[[...]]` pattern runs, or the plain pattern would consume the inner
    // `[[tag]]` first and leave a stray literal `#` behind. `#tagname` runs
    // last since it's the most general pattern and, by the time it runs, the
    // other two have already turned their matches into non-text nodes that
    // it can't touch anyway.
    findAndReplace(
      tree,
      [
        HASH_BRACKET_TAG_PATTERN,
        (_matched: string, rawTag: string) => ({ type: MDAST_TYPE, value: rawTag, form: "hashBracket" }) as never,
      ],
      { ignore: IGNORED_ANCESTOR_TYPES }
    );
    findAndReplace(
      tree,
      [WIKI_LINK_PATTERN, (_matched: string, rawTitle: string) => ({ type: MDAST_TYPE, value: rawTitle }) as never],
      { ignore: IGNORED_ANCESTOR_TYPES }
    );
    findAndReplace(
      tree,
      [
        HASH_TAG_PATTERN,
        (_matched: string, rawTag: string) => ({ type: MDAST_TYPE, value: rawTag, form: "hash" }) as never,
      ],
      { ignore: IGNORED_ANCESTOR_TYPES }
    );
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any -- $remark's generics
// can't express "an attacher whose `this` is a plain data bag", so the factory is
// handed through untyped; the attacher body above is still fully typed.
export const wikiLinkRemarkPlugin = $remark("wikiLink", (() => wikiLinkRemarkAttacher) as any);

/** Node schema for the rendered chip. Inline, atomic (not directly editable text), selectable as a whole. */
export const wikiLinkSchema = $nodeSchema(NODE_NAME, () => ({
  inline: true,
  group: "inline",
  atom: true,
  selectable: true,
  draggable: false,
  marks: "",
  attrs: {
    title: { default: "", validate: "string" },
    // Which surface syntax produced this chip -- "bracket" (`[[Title]]`,
    // the default), "hashBracket" (`#[[multi word tag]]`), or "hash"
    // (`#tagname`). Only read by the to-markdown serializer; it has no
    // bearing on rendering or click behavior, so a tag chip is genuinely
    // pixel-identical to a link chip (per the ticket's "no distinct visual
    // style").
    form: { default: DEFAULT_FORM, validate: "string" },
  },
  parseDOM: [
    {
      tag: "span[data-wiki-link-title]",
      getAttrs: (dom) => {
        if (!(dom instanceof HTMLElement)) return false;
        return {
          title: dom.getAttribute("data-wiki-link-title") ?? "",
          form: (dom.getAttribute("data-wiki-link-form") as LinkForm | null) ?? DEFAULT_FORM,
        };
      },
    },
  ],
  toDOM: (node) => [
    "span",
    {
      class: "wiki-link-chip",
      "data-wiki-link-title": node.attrs.title as string,
      "data-wiki-link-form": node.attrs.form as string,
    },
    node.attrs.title as string,
  ],
  parseMarkdown: {
    match: (node) => node.type === MDAST_TYPE,
    runner: (state, node, type) => {
      state.addNode(type, {
        title: (node.value as string | undefined) ?? "",
        form: (node.form as LinkForm | undefined) ?? DEFAULT_FORM,
      });
    },
  },
  toMarkdown: {
    match: (node) => node.type.name === NODE_NAME,
    runner: (state, node) => {
      state.addNode(MDAST_TYPE, undefined, node.attrs.title as string, { form: node.attrs.form as LinkForm });
    },
  },
}));

/** Converts a just-typed `[[Title]]` into a chip node live, as the user types. */
export const wikiLinkInputRule = $inputRule(
  (ctx) =>
    new InputRule(/\[\[([^[\]\n]+)\]\]$/, (state, match, start, end) => {
      const rawTitle = match[1];
      if (rawTitle === undefined) return null;
      const node = wikiLinkSchema.type(ctx).create({ title: rawTitle, form: "bracket" satisfies LinkForm });
      return state.tr.replaceWith(start, end, node);
    })
);

/** Converts a just-typed `#[[multi word tag]]` into a chip node live, as the user types. */
export const hashBracketTagInputRule = $inputRule(
  (ctx) =>
    new InputRule(/#\[\[([^[\]\n]+)\]\]$/, (state, match, start, end) => {
      const rawTag = match[1];
      if (rawTag === undefined) return null;
      const node = wikiLinkSchema.type(ctx).create({ title: rawTag, form: "hashBracket" satisfies LinkForm });
      return state.tr.replaceWith(start, end, node);
    })
);

/**
 * Converts a just-typed `#tagname` into a chip node live, as the user types
 * the boundary character right after it (see `HASH_TAG_INPUT_PATTERN`) --
 * the boundary character itself is left in the document, untouched, right
 * after the new chip.
 */
export const hashTagInputRule = $inputRule(
  (ctx) =>
    new InputRule(HASH_TAG_INPUT_PATTERN, (state, match, start, end) => {
      const rawTag = match[1];
      const boundary = match[2];
      if (rawTag === undefined || boundary === undefined) return null;
      const node = wikiLinkSchema.type(ctx).create({ title: rawTag, form: "hash" satisfies LinkForm });
      return state.tr.replaceWith(start, end - boundary.length, node);
    })
);

/** All plugins this feature registers, ready to pass to `Editor#use`. */
export const wikiLinkPlugins = [
  wikiLinkSchema,
  wikiLinkInputRule,
  hashBracketTagInputRule,
  hashTagInputRule,
  wikiLinkRemarkPlugin,
].flat();
