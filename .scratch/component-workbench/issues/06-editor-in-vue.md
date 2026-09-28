# Editor in Vue: wrap `PageEditor` or adopt `@milkdown/vue`

Type: prototype
Status: open
Blocked by: 04

## Question

Surfaces are Vue 3 components ([Framework or not: how surfaces are built](04-framework-decision.md)). How should the page editor (Milkdown + `wikiLinkPlugins`) be mounted inside Vue?

1. **A thin Vue wrapper** around the existing `PageEditor` class (`src/page-editor.ts`): construct it in `onMounted`, `destroy()` it in `onUnmounted`, and bridge `onChange` / `onLinkClick` to emits and the page's markdown to a prop.
2. **The official `@milkdown/vue` binding** (`MilkdownProvider`, `<Milkdown>`, `useEditor`), with `wikiLinkPlugins` passed into the `Editor.make()` chain.

Build both, rough, in a throwaway Vue + Vite setup, and compare them on:
- **Glue:** how much code each needs for the things the app needs: load a page, swap to another page (`key` vs `replaceAll` vs `load()`), read markdown back / autosave via `listener`, `scrollToHeading`, and the delegated wiki-link click.
- **Convenience:** what the binding actually gives beyond the wrapper (access to the editor instance through `useInstance`, a loading state, lifecycle handling), and whether any of it is used.
- **Cost:** the binding's type-only `@milkdown/crepe` dependency (about 3.4 MB on disk; research found it absent from the bundle) and its release lock-step with `@milkdown/kit`.
- **Story fit:** how each looks as a workbench story fed sample markdown, with `onChange`/`onLinkClick` visible. Also: which sample documents the editor story should seed (plain prose, wiki-links to headings, long document for scrolling).

Background: [Ways to shape a surface](02-surface-shape-options.md) (the Milkdown section) and the editor entry in the [surface inventory](../research/03-surface-inventory.md).
