// The search modal's row shape, shared between `SearchModal.vue`
// (presentational) and `SearchModalContainer.vue`. Kept out of
// `vault-api.ts`: the trailing "Create page" row has no backend
// counterpart, it's purely a UI affordance built by the container. Its
// `result` shape mirrors `vault-api.ts`'s `SearchResult` structurally (a
// `SearchResult` satisfies it directly) without the presentational SFC
// having to import `vault-api` itself -- see spec.md#surface-contract's
// "presentational SFC never imports vault-api" rule.

/** One search hit row, tagged with which strict tier matched it (1 = title,
 * 2 = tag, 3 = BM25 body). `matchedTag` is set only for a tier-2 hit;
 * `snippet` otherwise holds the title (tier 1) or a best-matching-section
 * body excerpt with `\u0001…\u0001` marking the highlighted span (tier 3). */
export interface SearchResultEntry {
  id: string;
  title: string;
  tier: 1 | 2 | 3;
  snippet: string;
  inTrash: boolean;
  matchedTag?: string | null;
}

/** One rendered row in the search results list: either a real search hit,
 * or the trailing "Create page" action. */
export type SearchEntry = { kind: "result"; result: SearchResultEntry } | { kind: "create"; query: string };
