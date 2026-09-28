# Editor in Vue: wrap `PageEditor` or adopt `@milkdown/vue`

Type: prototype
Status: resolved
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

## Answer

**Wrap the existing `PageEditor` in a thin Vue SFC. Do not adopt `@milkdown/vue`.**

- **Shape:** one SFC (`<script setup lang="ts">`). It constructs `PageEditor` on its root element in `onMounted`, calls `load(markdown)`, and calls `destroy()` in `onUnmounted`. It turns `onChange` / `onLinkClick` into typed emits (`change`, `linkClick`). Props are the page's identity plus its markdown, e.g. `pageKey` + `markdown`. Only a change of **page identity** reloads, by watching `pageKey` and calling `load()`. `markdown` changing because the parent echoes the SFC's own `change` back must not reload, since that would reset the cursor. `scrollToHeading` / `getMarkdown` stay available through `defineExpose`, subject to the [Surface contract](07-surface-contract.md). `PageEditor` itself stays unchanged.
- **Why not the binding:**
  - **Glue:** the wrapper took 33 lines; the binding took 86 across two components. The binding forces a provider/child split, because `useEditor` injects from `MilkdownProvider`. It also means writing the `Editor.make()` chain again (heading-id generator, listener, `wikiLinkPlugins`), plus the delegated wiki-link click and `scrollToHeading`, because `PageEditor` owns all of those inside `load()`.
  - **Convenience:** the binding adds about 60 lines of mount/unmount handling and a `loading` ref. Only `loading` would be used.
  - **Page swap:** the binding reads its setup once, so swapping pages means remounting with `:key`. That tears down and rebuilds, exactly like `load()`, so the binding gains nothing there.
  - **Cost:** `@milkdown/vue` pins `@milkdown/kit` to an exact version (installing it forced `7.22.1 → 7.22.2`), and it pulls in `@milkdown/crepe` (3.7 MB on disk).
- **Story fit:** both variants show the same props/emits, so the story is the same either way: `markdown` in, `change` / `linkClick` shown as actions. The editor story seeds three sample documents, one named story each:
  - **Plain prose:** headings, bold, italic, inline code, a list, and no links.
  - **Wiki-links:** a page link, a link to a heading on another page, a link to a page that doesn't exist yet, and a link to a heading on the same page.
  - **Long document:** about a dozen sections, for scroll-to-heading.
- **Finding, handed to [Surface contract](07-surface-contract.md):** Milkdown's `listener` plugin waits 200 ms after an edit before calling `markdownUpdated`, and `destroy()` cancels that pending call. `main.ts`'s `flushPendingSaveForCurrentPage` returns early when `saveTimer === null`, so an edit made in the last ~200 ms before a page switch is dropped, both today and in either variant. This was reproduced in the prototype. The editor surface's contract should make it send its final `change` itself before a page swap or unmount, rather than the parent reading the markdown back with `getMarkdown()`.

**Prototype (primary source):** branch `prototype/editor-in-vue`, commit `909c4ea`. Run it with `npm run prototype:editor-in-vue` and open `/prototype-editor-in-vue.html?variant=A|B` (port 5199). Variant A is `src/prototype-editor-in-vue/VariantA_WrapPageEditor.vue`; variant B is `VariantB_MilkdownVue.vue` + `VariantB_Inner.vue`. The harness is `App.vue`.
