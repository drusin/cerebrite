// `ArticleSurface.vue`'s backlink row shape, kept out of `vault-api.ts`'s
// import path for the presentational SFC (spec.md#surface-contract --
// "presentational SFC never imports vault-api") -- see
// `surfaces/search/search-entry.ts` for the same pattern. `vault-api.ts`'s
// `BacklinkEntry` satisfies this structurally, so
// `ArticleContainer.vue` (which does import `vault-api`) passes it straight
// through without any conversion.
export interface BacklinkEntry {
  sourceId: string;
  sourceTitle: string;
  snippet: string;
  modifiedAt: number;
  targetHeadingSlug?: string | null;
}
