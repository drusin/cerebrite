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

export class PageEditor {
  #root: HTMLElement;
  #onChange: (markdown: string) => void;
  #editor: Editor | null = null;

  constructor(root: HTMLElement, onChange: (markdown: string) => void) {
    this.#root = root;
    this.#onChange = onChange;
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
      .create();

    editor.action((ctx) => {
      ctx.get(listenerCtx).markdownUpdated((_ctx, nextMarkdown, prevMarkdown) => {
        if (nextMarkdown !== prevMarkdown) {
          this.#onChange(nextMarkdown);
        }
      });
    });

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
    if (this.#editor) {
      const editor = this.#editor;
      this.#editor = null;
      await editor.destroy();
    }
  }
}
