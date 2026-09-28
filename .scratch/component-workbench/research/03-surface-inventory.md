# Surface inventory and coupling map

Source of truth at the time of writing: `src/main.ts` (3840 lines), `index.html`, `src/page-editor.ts`, `src/wiki-link-plugin.ts`, `src/sync-status.ts`, `src/connect-wizard.ts`, `src/clone-wizard.ts`, `src/vault-api.ts`. Line numbers refer to `src/main.ts` unless stated. Facts only, no redesign.

## Module-level state in `main.ts`

| State | Line | Written by | Read by |
|---|---|---|---|
| `currentPage: OpenPage \| null` (persisted `{id, inTrash, trashedFilename}` or dynamic `{title}`) | 302 | `openResolution`, `flushSave` (dynamic → persisted), delete, restore, empty trash, change folder | sidebar highlight (page list + Recent), article, editor autosave, rename, `selectPage`, empty trash |
| `saveTimer` | 303 | `scheduleAutosave`, `flushSave` | `flushPendingSaveForCurrentPage` |
| `pageEditor: PageEditor \| null` (one instance, created lazily) | 304 | `renderPageArticle` | `flushPendingSaveForCurrentPage`, `openResolution` (scroll), rename reload |
| `currentVaultPath` | 306 | `openVaultAndLoad`, `finishCloneWizardIntoWorkspace`, clone manual submit | `openSettingsModal` |
| `currentSyncStatus` | 310 | `applySyncIndicator` only | `updateSyncSectionNotConnectedBanner` (Settings) |
| `recentPages: RecentEntry[]` (in-memory, cap 10) | 332 | `recordRecentOpen` (via `openResolution`), `pruneRecentEntries` (delete, empty trash), `flushSave` (materialize rewrites key), rename (title), change folder (clear) | `renderRecentList` |
| `searchEntries`, `searchSelectedIndex`, `searchDebounceTimer`, `searchRequestId` | 932–936 | search only | search only |
| `pendingGithubTokenPair` | 258 | Settings GitHub raw form | Settings GitHub raw form |
| `settingsSshKey` | 1797 | Settings SSH raw form | Settings SSH raw form |
| `wizardState`, `wizardAccessToken`, `wizardRefreshToken`, `wizardAccessTokenExpiresAt`, `wizardRepos`, `wizardSshKey`, `wizardGeneration`, `wizardPendingAccessTokenConnect` | 2123–2131, 2614 | connect wizard only | connect wizard only |
| `cloneWizardState`, `cloneWizardAccessToken`/`RefreshToken`/`AccessTokenExpiresAt`, `cloneWizardRepos`, `cloneWizardSshKey`, `cloneWizardPendingAccessTokenConnect`, `cloneWizardAuthorPrefill`, `cloneWizardGeneration`, `cloneWizardClonedPath` | 2779–2790, 2837 | clone wizard only | clone wizard only |
| `cloneManualGithubToken`, `cloneManualGitlabToken`, `cloneManualSshKey`, `cloneManualGeneration` | 3361–3367 | clone manual only | clone manual only |
| `COMPACT_MEDIA_QUERY` (`max-width: 700px`) | 3644 | — | `applyLayoutMode` |

Other shared state kept **in the DOM** rather than in variables: `hidden` on `#vault-picker` / `#workspace` / each overlay (the "is X open" checks read the attribute), `.sidebar-collapsed` and `.compact` on `#workspace`, `.drawer-open` on `#sidebar`, `data-theme` on `<html>`, the `.active` class on list buttons, and the form field values (e.g. the Sync section's URL inputs are written by `openSyncManualForm`).

## Backend seams

- **`vault-api.ts`**: every Tauri `invoke`, plus the one event subscription (`onSyncStatusChanged` → `listen("sync-status-changed")`). It is the only module that imports `@tauri-apps/api`.
- **`@tauri-apps/plugin-dialog`** (`confirm`, `message`), imported **directly in `main.ts`** (line 93), *not* through `vault-api`. Used by delete, rename, restore, empty trash, new page, change folder, create from search, and `maybeOfferProviderCommitAuthorSwitch`.
- **Browser natives used as dialogs**: `window.prompt` (new-page title, rename, three copies of SSH-key import), `window.confirm` (plaintext-fallback consent, store as plaintext, disconnect, remove all credentials), and `window.alert` (unlock failure, set up keychain, conflict-backups failure, disconnect result, orphan-cleanup failure).
- **`pickVaultFolder`** (a native folder picker, through `vault-api`) is reused as a plain directory picker by the vault picker, Settings' change-folder, the clone wizard's destination step, and the clone manual form.

## Already-pure modules

- `sync-status.ts`: imports only *types* from `vault-api`. `syncIndicatorFor(status)` returns `{glyph, iconState, statusText, severity, ctas}`. `ctasFor` and `describeSyncFailureCause` are exhaustive over causes.
- `connect-wizard.ts`, `clone-wizard.ts`: pure reducers (`reduceWizard` / `reduceCloneWizard`) plus `isDone`, `isSwitchedToManual`, and `isBusyStep`. No DOM, no `invoke`. `main.ts` owns rendering and all side effects.
- `heading-slug.ts`: pure.
- `wiki-link-plugin.ts`: imports only Milkdown/ProseMirror and mdast utilities, with **no `vault-api` and no page index**. It is purely syntax: it parses `[[x]]`, `#tag`, and `#[[multi word]]` into one chip node carrying `data-wiki-link-title`, and round-trips them exactly. Chips look the same whether or not the target exists. Links are resolved only on click, through a callback.
- `page-editor.ts`: `new PageEditor(root, onChange(markdown), onLinkClick(title))` with `load(markdown)`, `scrollToHeading(slug)`, `getMarkdown()`, and `destroy()`. It is already a mount/update/destroy-shaped surface with no backend dependency.

## Surfaces

### 1. Vault picker

- **DOM:** `#vault-picker` (`#select-vault-button`, `#clone-wizard-open-button`, `#clone-manual-open-button`, `#vault-picker-error`).
- **Functions:** `showVaultPicker(error?)` 473, `showWorkspace` 486, `handleSelectVaultClick` 907, `openVaultAndLoad` 897, `friendlyVaultOpenError` 463.
- **State:** writes `currentVaultPath` (via `openVaultAndLoad`).
- **vault-api:** `pickVaultFolder`, `openVault`, and at startup `getSettings` (`init`).
- **States:** default; with a translated nested-repo error; with a raw error, which includes the Android "no folder picker" rejection.
- **Cross-surface:** on success → `showWorkspace`, then `loadPages`, `loadTrash`, `refreshSyncStatus`, and writes Settings' `#settings-vault-path` text. Opens the clone wizard and the clone manual form. The clone wizard hides and re-shows this section directly.

### 2. Sidebar: layout chrome (collapse rail, compact drawer)

- **DOM:** `#sidebar`, `.sidebar-top` buttons, `#sidebar-rail` (expand, search, new page, settings, rail sync icon), `#sidebar-open-button`, `#sidebar-overlay`, `#sidebar-close-button`, `#sidebar-collapse-toggle`.
- **Functions:** `applyLayoutMode` 3651, `closeSidebarDrawer` 3646, inline listeners 3797–3812.
- **State:** DOM classes only (`sidebar-collapsed`, `compact`, `drawer-open`), plus `COMPACT_MEDIA_QUERY`.
- **vault-api:** none.
- **States:** expanded desktop; collapsed rail; compact with drawer closed; compact with drawer open.
- **Cross-surface:** buttons open search, Settings, new page, and the sync popup.

### 3. Sidebar: "All pages" list + New page + Today

- **DOM:** `#page-list`, `#new-page-button`, `#today-button` (plus rail `#sidebar-rail-new-page`).
- **Functions:** `renderPageList(pages)` 491, `loadPages` 733, `handleNewPageClick` 878, `handleTodayClick` 729, `todaysDateTitle` 714, `highlightActivePage` 434.
- **State:** reads `currentPage` (the active highlight, in both `renderPageList` and `highlightActivePage`).
- **vault-api:** `listPages`, `createPage`, and `resolvePage` (Today, through `openPageByTitle`). Uses `window.prompt` and `messageDialog`.
- **States:** empty; populated; one active page; a long list.
- **Cross-surface:** click → `selectPage` (article). New page → `selectPage`. `loadPages()` is called as a "pages changed" refresh from rename, delete, restore, materialize, create-from-search, clone finish, and vault open.

### 4. Sidebar: Recent list

- **DOM:** `#recent-list`, `#recent-empty`.
- **Functions:** `renderRecentList` 351, `recordRecentOpen` 335, `pruneRecentEntries` 343.
- **State:** owns `recentPages`, but five other code paths mutate it (see the state table). Reads `currentPage` through `highlightActivePage`.
- **vault-api:** none directly.
- **States:** empty ("No pages opened yet."); populated with persisted and dynamic entries; active entry.
- **Cross-surface:** click → `selectPage` (persisted) or `openPageByTitle` (dynamic).

### 5. Sidebar: Trash list + Empty trash

- **DOM:** `#trash-list`, `#empty-trash-button`.
- **Functions:** `renderTrashList(pages)` 739, `loadTrash` 754, `handleEmptyTrashClick` 854.
- **State:** empty trash reads and clears `currentPage` (if the open page is trashed) and prunes `recentPages`.
- **vault-api:** `listTrashedPages`, `emptyTrash`, and `resolvePage` (on click). Uses `confirmDialog` and `messageDialog`.
- **States:** empty (the list renders nothing, with no hint); populated.
- **Cross-surface:** click → `openPageByTitle` (the article in its in-trash state). Empty trash can hide the article.

### 6. Page article: title row, trash banner, backlinks

- **DOM:** `#page-view`, `#page-view-empty`, `#page-article`, `#page-title`, `#rename-page-button`, `#delete-page-button`, `#page-trash-banner`, `#restore-page-button`, `#backlinks-section` / `#backlinks-list` / `#backlinks-empty`.
- **Functions:** `renderPageArticle(title, body, headingSlug?, {deletable, pageId, inTrash, trashedFilename})` 573, `renderBacklinks(title)` 517, `openResolution` 641, `selectPage` 695, `openPageByTitle` 702, `handleDeletePageClick` 764, `handleRenamePageClick` 793, `handleRestoreClick` 841. `renderPageArticle` is close to a props-driven render already, but it also fetches backlinks itself.
- **State:** reads and writes `currentPage`, `pageEditor`, `recentPages`.
- **vault-api:** `getPage`, `resolvePage`, `getBacklinks` (render, plus a count before renaming), `renamePage`, `trashPage`, `restorePage`. Uses `window.prompt`, `confirmDialog`, `messageDialog`.
- **States:** nothing selected; persisted page (rename and delete visible); dynamic page (no rename or delete, empty body); in-trash page (banner and Restore, no delete); backlinks empty; backlinks grouped by source, with and without a `→ Heading` label; heading-targeted open (scroll).
- **Cross-surface:** backlink click → `selectPage`. Rename → `loadPages`, rewrites `recentPages`, reloads the editor. Delete → `loadPages`, `loadTrash`, `pruneRecentEntries`. Restore → `loadPages`, `loadTrash`, `selectPage`.

### 7. Page editor (Milkdown + wiki-link plugin)

- **DOM:** `#page-body` (`.milkdown-root`), owned by the `PageEditor` instance.
- **Functions:** the `PageEditor` class. Its glue in `main.ts` is `scheduleAutosave` 418, `flushSave` 389, and `flushPendingSaveForCurrentPage` 427, with a 1500 ms debounce.
- **State:** the glue reads `currentPage` and `saveTimer`. `flushSave` turns a dynamic `currentPage` into a persisted one.
- **vault-api:** none in the editor itself. The glue calls `savePage` and `materializeAndSavePage`.
- **States:** empty doc; plain markdown; headings (ids from `slugifyHeadingText`); `[[Link]]`, `[[Page#Heading]]`, `#tag`, and `#[[multi word tag]]` chips; long doc with scroll-to-heading.
- **Cross-surface:** `onLinkClick` → `openPageByTitle`. Materializing a page → `renderRecentList`, `loadPages`. The editor never needs a page index: chips don't show whether their target exists.

### 8. Search modal

- **DOM:** `#search-modal-overlay`, `#search-input`, `#search-include-trash`, `#search-results-list`, `#search-empty-hint`.
- **Functions:** `openSearchModal` 1089, `closeSearchModal` 1102, `runSearch` 1044, `scheduleSearch` 1077 (120 ms), `renderSearchEntries` 975, `renderHighlightedSnippet` 941, `moveSearchSelection`, `activateSelectedSearchEntry`, `handleSearchModalKeydown` 1128, `openSearchResult` 954, `handleCreatePageFromSearch` 960.
- **State:** only its own four variables.
- **vault-api:** `searchPages`, `createPage`, and `resolvePage` (through `openPageByTitle`).
- **States:** empty query ("Type to search."); no results, which shows only "Create page: '…'"; tier 1 title hit; tier 2 hit with a `#tag` chip; tier 3 hit with a `<mark>` snippet; an in-trash result with its badge; include trash on or off; keyboard selection moved; exact title match (no Create row).
- **Cross-surface:** result → `openPageByTitle`. Create → `loadPages`, `selectPage`. Opened by the sidebar button, the rail button, and a global Ctrl/Cmd+K listener in `init`.

### 9. Sync indicator + popup

- **DOM:** footer `#sync-status-button` / `-icon` / `-label`; rail `#sidebar-rail-sync-status` / `#sync-status-icon-rail`; `#sync-popup` with `-icon`, `-status-text`, `-provider`, `-last-synced`, `-cause-actions`, `-sync-now-button`, `-settings-link`.
- **Functions:** `applySyncIndicator(status)` 1482, `refreshSyncStatus` 1528, `open`/`close`/`toggleSyncPopup` 1545–1570 (positioned from the anchor's bounding rect), `refreshSyncPopupDetails` 1575, `handleSyncNowClick` 1602, `renderSyncPopupCtas` 1624, `handleSyncCtaClick` 1641, plus the CTA handlers 1667–1785: `openQuickReconnect`, `syncSubformKindFor`, `handleUnlockAndRetryClick`, `handleSetUpKeychainClick`, `handleStoreAsPlaintextClick`, `handleOpenConflictBackupsClick`. Document-level listeners close the popup on an outside click and on Escape (3700–3716).
- **State:** the only writer of `currentSyncStatus`.
- **vault-api:** `getSyncStatus`, `onSyncStatusChanged`, `getSyncDetails` (when the popup opens, and in reconnect), `triggerSyncNow`, `unlockKeychainAndRetrySync`, `revealConflictBackups`. Uses `window.alert` and `window.confirm`.
- **States:** icon states `notConnected`, `syncing`, `synced`, `retrying` (transient), and `needsAttention`, plus a `warning` severity (`refreshedSignInNotSaved`). The popup shows or omits provider and last-synced. There are 11 `SyncFailureCause` variants, each with its own text and 0–2 CTAs (for example, `keychainUnavailable` has two).
- **Cross-surface:** `applySyncIndicator` calls `updateSyncSectionNotConnectedBanner` (the Settings DOM). "Sync settings…" → `openSyncManualForm("")`. The "reconnect" and "store as plaintext" CTAs → `openQuickReconnect` → `openSyncManualForm(url)`, which also sets Settings' `#sync-credential-kind`. `refreshSyncStatus` is called from vault open, clone wizard finish, disconnect, and remove-all.
- `confirmSshHostKey` exists in `vault-api` but nothing in `main.ts` calls it. Neither `hostKeyUnconfirmed` nor `hostKeyMismatch` has a confirm UI; both map to the "reconnect" CTA.

### 10. Settings modal and its sub-forms

- **Shell:** `#settings-modal-overlay`. `openSettingsModal` 1880 (sets the vault path text, `refreshCommitAuthorFields`, `refreshOrphanNotice`), `closeSettingsModal` 3624, `isSettingsModalOpen`. Ctrl/Cmd+, toggles it and Escape closes it (a global keydown in `init`). Clicking the backdrop closes it.
- **Openers:** gear, rail gear, shortcut, `openSyncManualForm` (sync popup, connect wizard's switch to manual, quick reconnect), and clone manual success.
- **Sub-sections:**
  - **Vault folder:** `#settings-vault-path`, `#settings-change-folder-button`. `handleChangeVaultFolderClick` 1188 calls `pickVaultFolder`, flushes the editor save, clears `currentPage` and `recentPages`, hides the article, closes Settings, then runs `openVaultAndLoad`. This touches four other surfaces.
  - **Color scheme:** the `settings-theme` radios. `applyTheme` 1157 (sets `data-theme` on `<html>`), `handleThemeRadioChange` (calls `setTheme`), `setThemeRadioValue` (called at init from `getSettings`).
  - **Commit as:** `#settings-commit-author-form`. `refreshCommitAuthorFields` 2049 (`getCommitAuthor` → `commitAuthorPrefill`), `handleCommitAuthorFormSubmit` 2069 (`confirmCommitAuthor`, with a warning shown beside "Saved."), `maybeOfferProviderCommitAuthorSwitch` 2096 (`confirmDialog`). Both wizards also call `refreshCommitAuthorFields`.
  - **Connect a repository:** `#connect-wizard-open-button` → `openConnectWizard`.
  - **Sync (raw forms):** `#sync-section`. `updateSyncSectionNotConnectedBanner` 1515 (reads `currentSyncStatus`, shows either "Not connected, Connect…" or "Disconnect…"), `updateSyncSubformVisibility` 1461 (`#sync-credential-kind` → `data-sync-kind`), `openSyncManualForm(url)` 1869 (opens Settings, pre-fills four URL inputs, scrolls to the section). There are four raw sub-forms:
    - access token: `handleConnectFormSubmit` 1220 → `connectAccessToken`.
    - SSH key: generate, import, and connect at 1799–1858 → `generateSshKey`, `importSshKey`, `connectSshKey`. State lives in `settingsSshKey`.
    - GitHub: `handleGithubFormSubmit` 1254 (the device-flow poll loop), `finishGithubConnect` 1329 (`checkGithubInstallation` → install CTA → `connectGithubOauth`), `handleGithubInstallContinueClick`. State lives in `pendingGithubTokenPair`.
    - GitLab: `handleGitlabFormSubmit` 1382 → `startGitlabDeviceFlow`, `pollGitlabDeviceFlow`, `connectGitlabOauth`.

    All four connect calls go through `withPlaintextFallbackConsent` 1724 (`window.confirm`, `PLAINTEXT_CONSENT_REQUIRED_ERROR`). Disconnect is `handleDisconnectClick` 1958 → `disconnectVault`. It shows `disconnectResultMessage` 1929 (pure) through `window.alert`, then calls `refreshSyncStatus`.
  - **Credentials:** `#settings-orphan-notice`. `refreshOrphanNotice` 1982 (`scanOrphanedConnections`, also called at init), `handleOrphanCleanupClick` (`cleanupOrphanedConnections`), `handleRemoveAllCredentialsClick` 2008 (`removeAllStoredCredentials`, then refreshes the orphan notice and sync status).
- **States:** no vault path; the commit author prefilled / saved / saved with a warning / showing an error; not-connected banner vs Disconnect button; each sub-form visible; per-form status text ("Connecting…", "Connected.", or an error); GitHub device code shown; GitHub install CTA; orphan notice shown or hidden; remove-all fully succeeded vs partially failed.

### 11. Connect wizard (modal over Settings)

- **DOM:** `#connect-wizard-overlay`. `#connect-wizard-body` is rebuilt for each step. The footer has `#connect-wizard-back-button` and `#connect-wizard-manual-button`.
- **Functions:** `dispatchWizard` 2144 (reducer → render; on switched-to-manual → `handleWizardManualSetupClick`; on done → close), `openConnectWizard` 2160 (resets all wizard variables), `closeConnectWizard`, `renderWizardStep` 2199 (one render function per step 2245–2764), and the effects `runOauthSignIn` 2342, `handleCreateRepoClick` 2459, `loadRepoPicker` 2506, `handleWizardGenerateSshKey` / `ImportSshKey`, `performWizardConnect` 2661. The body is built with the local `el()` helper 2133. Staleness is handled with `wizardGeneration`.
- **State:** all its own variables.
- **vault-api:** `startGithubDeviceFlow`, `startGitlabDeviceFlow`, `pollGithubDeviceFlow`, `pollGitlabDeviceFlow`, `checkGithubInstallation`, `connectGithubOauth`, `connectGitlabOauth`, `connectAccessToken`, `connectSshKey`, `createGithubRepository`, `createGitlabRepository`, `listGithubRepositories`, `listGitlabRepositories`, `generateSshKey`, `importSshKey`, `getCommitAuthor`, `commitAuthorPrefill`, `confirmCommitAuthor`. This is the heaviest backend user.
- **States:** steps `hasRepo`, `providerChoice` (with "Another provider" only when `hasRepo`), `credentialChoice`, `otherCredentialKindChoice`, `oauthSignIn` (requesting, code shown, error), `repoVisibility` (with a create error), `repoPicker` (loading, empty, list, error), `pasteUrl` (token variant and SSH variant, with validation errors), `connecting`, `connectError`, `commitAuthor` (prefilled, error). Back is hidden on `hasRepo` and busy steps.
- **Cross-surface:** switch to manual → `openSyncManualForm(remoteUrl)`. The `commitAuthor` step → `refreshCommitAuthorFields` (Settings). Opened from Settings (two buttons). Escape and the backdrop close it.

### 12. Clone wizard (full-screen)

- **DOM:** `#clone-wizard-overlay`. `#clone-wizard-body` is rebuilt for each step. There are back and manual buttons.
- **Functions:** `dispatchCloneWizard` 2792, `openCloneWizard` 2839 (hides `#vault-picker`), `closeCloneWizard` 2857 (re-shows `#vault-picker` unless done), `renderCloneWizardStep` 2866 (step renderers 2909–3347), and the effects `runCloneOauthSignIn` 2984, `loadCloneRepoPicker` 3065, `handleCloneDestinationPickClick` 3210, `performClone` 3236, `finishCloneWizardIntoWorkspace` 2825.
- **State:** all its own variables.
- **vault-api:** device-flow start and poll (both providers), `listGithubRepositories` / `listGitlabRepositories`, `checkGithubInstallation`, `generateSshKey` / `importSshKey`, `pickVaultFolder`, `cloneAndOpenVault`, `confirmCommitAuthor`. It is prefilled from `cloneAndOpenVault`'s `authorPrefill`, not from `commitAuthorPrefill`.
- **States:** steps `providerChoice`, `credentialChoice`, `otherCredentialKindChoice`, `oauthSignIn`, `repoPicker`, `pasteUrl` (token or SSH), `destinationPicker`, `cloning`, `cloneError`, `commitAuthor`.
- **Cross-surface:** done → sets `currentVaultPath` and the Settings path text, `showWorkspace`, `loadPages`, `loadTrash`, `refreshSyncStatus`. Switch to manual → `openCloneManualDialog(url, dest)`. The commit author step → `refreshCommitAuthorFields`. Escape closes it unless a step is busy.

### 13. Clone manual form

- **DOM:** `#clone-manual-overlay` (static markup: URL, disabled branch, destination and picker, `#clone-manual-credential-kind`, four `data-clone-kind` sub-forms, submit, status).
- **Functions:** `openCloneManualDialog(url?, dest?)` 3393, `closeCloneManualDialog`, `updateCloneManualSubformVisibility`, `resetCloneManualStatuses`, `showCloneManualError`, `handleCloneManualDestinationClick`, SSH generate and import, `runCloneManualOauthSignIn(provider)` 3455, `handleCloneManualSubmitClick` 3553.
- **State:** its own four variables.
- **vault-api:** `pickVaultFolder`, `generateSshKey`, `importSshKey`, device-flow start and poll (both providers), `checkGithubInstallation`, `cloneAndOpenVault`.
- **States:** each of the four sub-forms; pre-filled from the wizard; validation errors; device code shown; "Signed in. Click Clone to continue."; cloning; clone error.
- **Cross-surface:** success → sets `currentVaultPath` and the Settings path text, `showWorkspace`, `loadPages`, `loadTrash`, `openSettingsModal`. Unlike the other two vault-open paths, it does **not** call `refreshSyncStatus`, so the sidebar icon depends on the `sync-status-changed` event arriving.

### 14. Orphan notice

Not a separate surface. It is the `#settings-orphan-notice` paragraph in Settings' Credentials section (covered under Settings). It refreshes at init, when Settings opens, and after cleanup or remove-all.

### Not listed in the ticket

- **App shell / global wiring** (`init` 3659): three separate `window`/`document` keydown/click listeners (search shortcut, Settings shortcut and Escape, sync popup Escape and outside click), then `getSettings` → theme, then auto-opening the remembered vault or showing the picker.
- **Native dialogs** (`plugin-dialog` plus `window.prompt`/`confirm`/`alert`) act as an implicit, un-rendered surface used by pages, trash, settings, and sync.

## Repeated patterns

These are facts about how the code is shaped today, relevant to surface boundaries:

- **The device-flow poll loop exists 5 times:** Settings GitHub (1289), Settings GitLab (1417), connect wizard (2373), clone wizard (3015), clone manual (3503).
- **SSH key generate/import exists 4 times:** Settings, connect wizard, clone wizard, clone manual.
- **The "Commit as" name/email form exists 3 times:** Settings, connect wizard step, clone wizard step.
- **The credential-kind selector with sub-form toggle exists twice:** Settings Sync (`data-sync-kind`) and clone manual (`data-clone-kind`).
- **Page lists are rendered 4 times with the same `<li><button>` shape:** page list, Recent, Trash, backlinks.

## Coupling summary

### Nearly isolated (pilot candidates)

- **Page editor:** already a class with a callback interface and no backend. Stories only need markdown and two callbacks. The plugin needs no page index. The most isolated surface there is, but it is a leaf rather than a migration of `main.ts` code.
- **Sync indicator + popup:** a pure derivation (`syncIndicatorFor`) already exists. Its state is one variable, which only it writes. The inputs are `SyncStatus`, plus `SyncDetails` for the popup. The outputs are about eight intents: sync now, open sync settings, reconnect, unlock, set up keychain, store as plaintext, open backups, and close. The couplings to cut are three: its direct write into Settings' Sync banner, `openQuickReconnect` reaching into Settings' `#sync-credential-kind`, and popup positioning reading the anchor element.
- **Search modal:** all its state is private. Its inputs are the query results. Its outputs are "open title" and "create page from query". It has one backend query, `searchPages`.
- **Vault picker:** trivial. The input is an optional error message. The outputs are pick, open clone wizard, and open clone manual.
- **Clone manual, clone wizard, connect wizard:** their state is private, and the two wizards already have pure reducers. Coupling outward is limited to a few calls: open manual form, refresh commit author, finish into workspace. They are the heaviest `vault-api` users, though: the device flow, repo lists, and connect/clone calls are all backend round trips inside the surface today.

### Tangled

- **Page-navigation cluster:** page list, Recent, Trash, article, backlinks, editor glue, and Settings' change folder. They share `currentPage`, `recentPages`, `pageEditor`, and `saveTimer`. `highlightActivePage` reaches into two lists' DOM, and five paths mutate `recentPages`. `loadPages()` and `loadTrash()` work as ad-hoc "pages changed" and "trash changed" refresh signals called from about eight places.
- **Settings:** a hub. It has six entry paths, hosts four raw connect forms plus Commit as plus Credentials, is written into from outside (the sync banner, URL pre-fills, the credential-kind select, the vault path text from three vault-open paths), and its change-folder action resets the page-navigation cluster.

### Truly shared app state vs accidentally global

- **Truly shared:**
  - `currentPage` (which page is open, read by the lists, article, and editor).
  - `recentPages` (fed by navigation, rename, trash, and materialize).
  - `currentVaultPath` and the "a vault is open" workspace/picker switch.
  - `currentSyncStatus` (the indicator and Settings' Sync banner).
  - The theme (`data-theme` on `<html>`, persisted through `setTheme`).
  - The page and trash lists as data, re-fetched on each change signal.
- **Accidentally global** (module-level only because everything lives in one file, and each used by exactly one surface): the search variables, all `wizard*`, all `cloneWizard*`, all `cloneManual*`, `pendingGithubTokenPair`, `settingsSshKey`, `saveTimer` (editor glue), and the `pageEditor` instance.
