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
import { findAndReplace } from "mdast-util-find-and-replace";
import type { Root as MdastRoot } from "mdast";
import { $inputRule, $nodeSchema, $remark } from "@milkdown/kit/utils";
import { InputRule } from "@milkdown/kit/prose/inputrules";

/** The mdast node "type" used for a parsed `[[...]]` run. */
const MDAST_TYPE = "wikiLink";

/** The ProseMirror node name registered in the schema. */
const NODE_NAME = "wiki_link";

/** Matches `[[<title>]]` where `<title>` is a single line with no nested brackets. */
const WIKI_LINK_PATTERN = /\[\[([^[\]\n]+)\]\]/g;

/** Node types whose text content should never be treated as a wiki-link source. */
const IGNORED_ANCESTOR_TYPES = ["code", "inlineCode"];

interface WikiLinkMdastNode {
  type: string;
  value?: string;
}

/**
 * `mdast-util-to-markdown` extension: stringifies a `wikiLink` mdast node
 * back to `[[<value>]]` with no escaping, so a value round-trips exactly as
 * typed regardless of what characters it contains.
 */
const wikiLinkToMarkdownExtension = {
  handlers: {
    [MDAST_TYPE]: (node: WikiLinkMdastNode) => `[[${node.value ?? ""}]]`,
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
    findAndReplace(
      tree,
      [WIKI_LINK_PATTERN, (_matched: string, rawTitle: string) => ({ type: MDAST_TYPE, value: rawTitle }) as never],
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
  },
  parseDOM: [
    {
      tag: "span[data-wiki-link-title]",
      getAttrs: (dom) => {
        if (!(dom instanceof HTMLElement)) return false;
        return { title: dom.getAttribute("data-wiki-link-title") ?? "" };
      },
    },
  ],
  toDOM: (node) => [
    "span",
    {
      class: "wiki-link-chip",
      "data-wiki-link-title": node.attrs.title as string,
    },
    node.attrs.title as string,
  ],
  parseMarkdown: {
    match: (node) => node.type === MDAST_TYPE,
    runner: (state, node, type) => {
      state.addNode(type, { title: (node.value as string | undefined) ?? "" });
    },
  },
  toMarkdown: {
    match: (node) => node.type.name === NODE_NAME,
    runner: (state, node) => {
      state.addNode(MDAST_TYPE, undefined, node.attrs.title as string);
    },
  },
}));

/** Converts a just-typed `[[Title]]` into a chip node live, as the user types. */
export const wikiLinkInputRule = $inputRule(
  (ctx) =>
    new InputRule(/\[\[([^[\]\n]+)\]\]$/, (state, match, start, end) => {
      const rawTitle = match[1];
      if (rawTitle === undefined) return null;
      const node = wikiLinkSchema.type(ctx).create({ title: rawTitle });
      return state.tr.replaceWith(start, end, node);
    })
);

/** All plugins this feature registers, ready to pass to `Editor#use`. */
export const wikiLinkPlugins = [wikiLinkSchema, wikiLinkInputRule, wikiLinkRemarkPlugin].flat();
