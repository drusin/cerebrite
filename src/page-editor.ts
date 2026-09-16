// Thin wrapper around a Milkdown instance (https://milkdown.dev), configured
// with just the commonmark preset -- no toolbar/slash-menu/theme plugin
// ecosystem, per issue 03 ("commonmark preset + a controlled
// markdown-in-markdown-out wrapper is enough").
//
// Milkdown's default serializer (remark-stringify under the hood) is what
// keeps saved files "clean markdown" (CONTEXT.md / ADR-0003): it does not
// inject HTML comments or inline block-id markers into the body, unlike
// some alternatives (see the prototype write-up in
// .scratch/cerebrite-mvp/issues/02-wysiwyg-editor-component.md for why
// Milkdown was chosen over Tiptap here).
import { Editor, defaultValueCtx, editorViewCtx, rootCtx, serializerCtx } from "@milkdown/kit/core";
import { commonmark, headingIdGenerator } from "@milkdown/kit/preset/commonmark";
import { history } from "@milkdown/kit/plugin/history";
import { listener, listenerCtx } from "@milkdown/kit/plugin/listener";
import { wikiLinkPlugins } from "./wiki-link-plugin";
import { slugifyHeadingText } from "./heading-slug";

export class PageEditor {
  #root: HTMLElement;
  #onChange: (markdown: string) => void;
  #onLinkClick: (title: string) => void;
  #editor: Editor | null = null;
  #handleClick: (event: MouseEvent) => void;

  constructor(root: HTMLElement, onChange: (markdown: string) => void, onLinkClick: (title: string) => void) {
    this.#root = root;
    this.#onChange = onChange;
    this.#onLinkClick = onLinkClick;

    // Delegated click handler for `[[Link]]` chips (issue 05): the chip is
    // rendered as an atomic ProseMirror node (see wiki-link-plugin.ts), so a
    // plain DOM click listener on the root -- rather than a custom NodeView
    // -- is enough to intercept clicks on it.
    this.#handleClick = (event: MouseEvent) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      const chip = target.closest<HTMLElement>("[data-wiki-link-title]");
      if (!chip) return;
      event.preventDefault();
      this.#onLinkClick(chip.dataset.wikiLinkTitle ?? "");
    };
  }

  /** Tears down any existing instance and mounts a fresh editor over `markdown`. */
  async load(markdown: string): Promise<void> {
    await this.destroy();
    this.#root.innerHTML = "";

    const editor = await Editor.make()
      .config((ctx) => {
        ctx.set(rootCtx, this.#root);
        ctx.set(defaultValueCtx, markdown);
        // Heading-level linking (issue 07 / ADR-0005): the editor is the
        // only place page content renders (there's no separate read-only
        // HTML view -- markdown.rs's HTML renderer is unused by the
        // frontend), so headings need addressable ids right inside the
        // ProseMirror DOM. Milkdown's commonmark preset already ships a
        // `headingIdGenerator` context slice plus a plugin
        // (`syncHeadingIdPlugin`) that keeps every heading node's `id`
        // attribute (and therefore its rendered `<h2 id="...">`) in sync
        // with its text, deduping repeats within the page automatically.
        // Overriding the generator to use the same base slug scheme as the
        // Rust side (`slugifyHeadingText`, mirroring
        // src-tauri/src/heading_slug.rs) makes a heading's DOM id match
        // exactly what a `[[Page#Heading]]` link resolves to, for the (by
        // far most common) case of no duplicate heading text on the page.
        // Milkdown's own dedup suffix format (`-#2`, `-#3`) differs from the
        // Rust side's (`-2`, `-3`) for the edge case of repeated headings --
        // left as a known, documented mismatch rather than replacing
        // Milkdown's built-in plugin, since duplicate heading text within one
        // page is rare and this can't be visually verified here anyway.
        ctx.set(headingIdGenerator.key, (node) => slugifyHeadingText(node.textContent));
      })
      .use(commonmark)
      .use(history)
      .use(listener)
      .use(wikiLinkPlugins)
      .create();

    editor.action((ctx) => {
      ctx.get(listenerCtx).markdownUpdated((_ctx, nextMarkdown, prevMarkdown) => {
        if (nextMarkdown !== prevMarkdown) {
          this.#onChange(nextMarkdown);
        }
      });
    });

    this.#root.addEventListener("click", this.#handleClick);

    this.#editor = editor;
  }

  /**
   * Scrolls the heading whose id matches `headingSlug` into view (ticket 07's
   * click-through-to-heading requirement), if one exists in the currently
   * mounted document. A no-op (returns `false`) when nothing matches -- e.g.
   * the link's heading fragment doesn't exist on this page -- so callers
   * don't need to special-case that themselves.
   */
  scrollToHeading(headingSlug: string): boolean {
    const target = this.#root.querySelector<HTMLElement>(`#${CSS.escape(headingSlug)}`);
    if (!target) return false;
    target.scrollIntoView({ behavior: "smooth", block: "start" });
    return true;
  }

  /** Serializes the current document back to markdown, or `null` if nothing is mounted. */
  getMarkdown(): string | null {
    if (!this.#editor) return null;
    return this.#editor.action((ctx) => {
      const view = ctx.get(editorViewCtx);
      const serializer = ctx.get(serializerCtx);
      return serializer(view.state.doc);
    });
  }

  async destroy(): Promise<void> {
    this.#root.removeEventListener("click", this.#handleClick);
    if (this.#editor) {
      const editor = this.#editor;
      this.#editor = null;
      await editor.destroy();
    }
  }
}
