// `RecentSurface.vue`'s entry shape, kept out of `state/pages.ts`'s import
// path for the presentational SFC (spec.md#surface-contract --
// "presentational SFC never imports a src/state/ module") -- see
// `surfaces/article/backlink-entry.ts` and `surfaces/search/search-entry.ts`
// for the same pattern. `state/pages.ts`'s `RecentEntry` satisfies this
// structurally, so `RecentContainer.vue` (which does import `state/pages.ts`)
// passes it straight through without any conversion.
export interface RecentEntry {
  /** Dedupe/identity key: `p:<id>` for a persisted page, `d:<normalizedTitle>` for a dynamic one. */
  key: string;
  title: string;
  kind: "persisted" | "dynamic";
  /** Set only for `kind === "persisted"`; used to navigate by id. */
  pageId?: string;
}
