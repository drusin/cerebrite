# 12: Sync status sidebar indicator

**What to build:** Per [ticket 09](../../git-provider-integration/issues/09-sync-and-auth-failure-surfacing.md), a persistent, quiet icon in the sidebar footer reading the `get_sync_status`/`sync-status-changed` machinery that already exists on the Rust side but that no UI currently reads.

From the user's perspective: a small, calm icon always sits in the sidebar footer, in one of five states — not connected, synced, syncing, retrying, needs attention (the specific needs-attention messaging/CTAs are ticket 13; this ticket just needs the icon to reach that state and open a popup). Clicking it opens a small popup showing the provider, last-synced time, current status text, and a link into Settings › Sync. A "Sync now" action is always available, independent of the background timer. Nothing here is a toast or unprompted interruption.

**Blocked by:** 03

- [ ] Sidebar footer icon with five distinct visual states: not connected, synced, syncing, retrying, needs attention
- [ ] Icon reads `get_sync_status` on load and updates live from `sync-status-changed`
- [ ] Clicking opens a popup: provider name, last-synced timestamp, current status text, a link to Settings › Sync — never an auto-opening toast/banner
- [ ] "Sync now" is available in the popup (and/or Settings › Sync), triggers an immediate sync attempt independent of the background timer
- [ ] "Not connected" state (no remote at all) uses its own calm icon and never nags — Settings › Sync shows a plain "Not connected — connect a repository" with the connect action
- [ ] "Retrying" (transient, from ticket 03's classification) is visually distinct from "needs attention" and shows no CTA
- [ ] Manual QA: toggling network off mid-sync shows retrying; a hardcoded bad credential (or ticket 04/05's rejected-token test) shows needs attention
