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
import { commonmark } from "@milkdown/kit/preset/commonmark";
import { history } from "@milkdown/kit/plugin/history";
import { listener, listenerCtx } from "@milkdown/kit/plugin/listener";
import { wikiLinkPlugins } from "./wiki-link-plugin";

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
