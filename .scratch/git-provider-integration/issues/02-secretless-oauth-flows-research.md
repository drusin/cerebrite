# Research: Secret-Free OAuth for a Distributed Desktop Git Client (Cerebrite)

(Full research subagent output for ticket 02 — secret-free OAuth flows.)

**Note on methodology/limitations:** Direct `web_fetch` access to the public internet was blocked in this environment for every attempt (docs.github.com, github.blog, docs.gitlab.com, tauri.app all returned "permission denied"), and the GitHub MCP server available was scoped to an internal GitHub Enterprise instance, not github.com or gitlab.com. All findings below come from an AI web-search tool that synthesizes and quotes primary-source pages with citations. Treat single-sourced or date-sensitive claims (especially the 2025/2026 GitHub changelog items) as needing a final human eyeball-check against the live docs before implementation.

---

## 1. GitHub: Device Flow, PKCE, and GitHub App vs OAuth App

### Device Authorization Flow — exists, secret-free, but must be manually enabled
- GitHub supports the OAuth 2.0 Device Authorization Grant ("device flow") for both **OAuth Apps and GitHub Apps**. Flow: app POSTs `client_id` (+ scopes) to `POST /login/device/code`; GitHub returns `device_code`, `user_code`, `verification_uri` (`https://github.com/login/device`), and an expiry/interval; app shows the code/URL; app polls `POST /login/oauth/access_token` until the user finishes.
  Source: [Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)
- **This flow uses no client secret at all.**
- Since March 2022, device flow must be **explicitly enabled per-app** in the app's GitHub settings — off by default for anti-abuse reasons.
  Source: [Enable OAuth Device Authentication Flow for Apps](https://github.blog/changelog/2022-03-16-enable-oauth-device-authentication-flow-for-apps/)

### PKCE + Authorization Code flow, no redirect secret needed — but with a real redirect-URI wrinkle
- GitHub added first-class PKCE support (RFC 7636, S256 only) for both OAuth Apps and GitHub Apps on **2025-07-14**.
  Source: [PKCE support for OAuth and GitHub App authentication](https://github.blog/changelog/2025-07-14-pkce-support-for-oauth-and-github-app-authentication/)
- VS Code's own PKCE implementation (public client) **omits `client_secret`**, confirmed working against GitHub around Sept–Oct 2025.
  Sources: [microsoft/vscode#264795](https://github.com/microsoft/vscode/issues/264795), [microsoft/vscode#259642](https://github.com/microsoft/vscode/issues/259642)
- **Caveat — GitHub has no formal "public client" designator** the way GitLab does; it simply now *accepts* PKCE-only exchanges. De-facto rather than de-jure "no secret required," roughly a year old — verify directly before shipping.

### Redirect URI mechanics — a real cost for the PKCE/loopback path
- GitHub OAuth Apps historically required one **exact** callback URL match. On **2026-08-14**, GitHub added up to 10 redirect URIs per app plus optional wildcard *path* matching.
  Source: [Multiple redirect URIs and token refresh for OAuth apps](https://github.blog/changelog/2026-08-14-multiple-redirect-uris-and-token-refresh-for-oauth-apps/)
- **Wildcard matching does not extend to the port number** — GitHub does not support RFC 8252 §7.3's "any port" loopback behavior. Must pre-register exact `http://127.0.0.1:<port>/...` URL(s).
  Sources: [About the user authorization callback URL](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/about-the-user-authorization-callback-url), [modelcontextprotocol/typescript-sdk#1316](https://github.com/modelcontextprotocol/typescript-sdk/issues/1316), [better-auth/better-auth#11278](https://github.com/better-auth/better-auth/issues/11278)
- **Practical consequence:** a GitHub PKCE-loopback flow can't dynamically bind an OS-assigned ephemeral port — must pre-register a small pool of fixed local ports (up to 10) and try each in turn. Genuine friction the device flow entirely avoids.

### GitHub App vs. OAuth App — which to register as
- **OAuth App**: user-wide, all-or-nothing scopes (`repo` grants *every* repo); historically non-expiring tokens unless opted into short-lived mode (now default for newly-created OAuth Apps: 8h access token + 6-month refresh token).
  Sources: [Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps), [Token expiration and revocation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation)
- **GitHub App**: fine-grained, repo-selectable permissions ("Contents: Read and write" is the specific permission for git push/clone via a user-to-server token); tokens short-lived by design (~8h) with a refresh token (~6 months, single-use/rotated).
  Sources: [Permissions required for GitHub Apps](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps), [Refreshing user access tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)
- **Refresh without a client secret**: for a GitHub App registered/authenticated as a public client via PKCE, `grant_type=refresh_token` should not require re-presenting a secret — confirm against live docs before implementation.
- **Recommendation driver**: GitHub App is the better registration type — scoped to "Contents: Read and write" only (least privilege vs. OAuth App's blanket `repo`), and its mandatory short-lived-token + refresh model is actually a feature for a background-syncing app that needs a durable, silent, secret-free refresh path.

### Scopes and git usability (GitHub)
- OAuth App: `repo` (private+public) or `public_repo` (public only); token directly usable as HTTPS Basic-auth password (any non-empty username works, `x-access-token` conventional).
  Sources: [Scopes for OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps), [SO discussion](https://stackoverflow.com/questions/63906613/minimal-set-of-scopes-to-push-to-github-using-an-access-token)
- GitHub App user-to-server tokens: also directly usable, conventionally with username `x-access-token`.
  Sources: [github/orgs/community#173881](https://github.com/orgs/community/discussions/173881), [Easier builds and deployments using Git over HTTPS and OAuth](https://github.blog/news-insights/easier-builds-and-deployments-using-git-over-https-and-oauth/)
- Device flow token carries the same scopes/usability as any other OAuth-App-issued token — just a different acquisition path.

---

## 2. GitLab (gitlab.com): PKCE, Device Flow, Confidential vs Public Applications

### Confidential vs. non-confidential ("public") applications — the load-bearing distinction
- Registering an "Application" on gitlab.com has an explicit **"Confidential"** checkbox. Checked = requires `client_secret`. Unchecked (public) = no secret issued/required, PKCE expected.
  Source: [Authenticate with GitLab | GitLab CLI Docs](https://docs.gitlab.com/cli/authentication/), [GitLab OAuth2 identity provider API](https://docs.gitlab.com/api/oauth2/)
- This is a more explicit, first-class "public client" model than GitHub's; GitLab's own CLI (`glab`) relies on and documents this directly.

### PKCE with loopback redirect — supported
- Non-confidential applications support Authorization Code + PKCE with a `http://localhost:<port>/...`-style redirect URI (registered as exact URI at app-creation, same general model as GitHub; no confirmed wildcard-port matching).
  Source: [OAuth2 identity provider API — PKCE section](https://docs.gitlab.com/api/oauth2/#authorization-code-with-proof-key-for-code-exchange-pkce)
- Legacy `urn:ietf:wg:oauth:2.0:oob` (paste-a-code) flow is deprecated industry-wide; don't use for new integration.

### Device Authorization Grant — supported since GitLab 17.1, applies to gitlab.com
- RFC 8628 device grant starting GitLab 17.1; gitlab.com runs current versions so it's available. Standard flow: device_code/user_code/verification_uri → user visits + enters code → app polls `/oauth/token`.
  Source: [OAuth 2.0 identity provider API — device grant](https://docs.gitlab.com/api/oauth2/#device-authorization-grant), corroborated by [GitLab CLI auth docs](https://docs.gitlab.com/cli/authentication/)
- App must have `device_code` enabled as allowed grant type at registration; should be non-confidential to avoid needing a secret.

### Token lifetime and refresh (GitLab)
- OAuth access tokens: short-lived, **2 hours (7200s)** by default.
- Refresh token issued alongside; using it **rotates** it; refresh tokens remain usable after the paired access token expires.
- Refresh works for public (PKCE) applications **without a client secret**.
  Sources: [Configure GitLab as an OAuth 2.0 identity provider](https://docs.gitlab.com/integration/oauth_provider/), [REST API authentication](https://docs.gitlab.com/api/rest/authentication/)

### Scopes and git usability (GitLab) — an important gotcha
- `read_repository`/`write_repository` exist plus the broad `api` scope.
- For Personal Access Tokens, `write_repository` suffices for `git push`.
- **For OAuth tokens specifically, `write_repository` alone does NOT work for `git push` over HTTPS** — GitLab requires the broader **`api`** scope to succeed, a known, still-open GitLab defect/gap.
  Sources: [gitlab-org/gitlab#321359](https://gitlab.com/gitlab-org/gitlab/-/issues/321359), [Access token scopes](https://docs.gitlab.com/security/tokens/access_token_scopes/)
- Token, once correctly scoped, usable as `git clone https://oauth2:<TOKEN>@gitlab.com/<namespace>/<repo>.git` (username `oauth2` is convention only, not validated).
  Sources: [gitlab-org/gitlab#212953](https://gitlab.com/gitlab-org/gitlab/-/issues/212953), [GitLab forum thread](https://forum.gitlab.com/t/cloning-a-private-project-repository-using-oauth2-token/55225)
- **Implication for Cerebrite:** working `git push` via GitLab OAuth requires the broad `api` scope, not `write_repository` — broader than we'd like, but it's what GitLab requires.

---

## 3. Redirect Mechanics vs. the Android-Possible Constraint

| Mechanism | What it requires | Desktop | Android |
|---|---|---|---|
| **Loopback redirect** | Bind local TCP listener, open browser, browser redirects to localhost | Works cleanly, no scheme registration | RFC 8252 §7.3 nominally endorses loopback but not the natural mobile pattern; app-lifecycle/foreground-listener issues make it awkward. Survives, but the weaker Android option. |
| **Custom URI scheme** / App Link | Register scheme/handler in manifest at build time | Windows/Linux: activates app, passes URL as argv (needs single-instance handling); macOS: `Info.plist`, build-time only | Supports custom schemes and verified App Links (needs static `assetlinks.json` hosting — a static file, not a live backend, so doesn't violate "no server-side" but needs a stable domain). Survives the Android-possible constraint. |
| **Device Authorization Grant** | Nothing — no listener, no scheme, no redirect URI | Works identically everywhere | Survives cleanly and trivially — most "Android-possible" by construction. |

Source for Tauri deep-link plugin behavior: Tauri's official deep-linking plugin docs (per-platform config-time registration on Windows/Linux/macOS/Android/iOS; Windows/Linux pass URL as argv; macOS is `Info.plist`-only; Android supports App Links + custom schemes; iOS supports Universal Links + custom schemes).
Sources: [Deep Linking — Tauri v2 Plugin Docs](https://v2.tauri.app/plugin/deep-linking/), [@tauri-apps/plugin-deep-link JS API](https://v2.tauri.app/reference/javascript/deep-link/) *(could not directly fetch these pages this session due to tool restrictions — verify against live docs before implementation)*.

**Bottom line on the constraint:** loopback-redirect is "Android-possible" only awkwardly; custom-scheme/App-Link is fully Android-possible via Tauri's own plugin; **device flow avoids the whole redirect-mechanics problem categorically** and is the cleanest fit for "Android-possible, not Android-now."

---

## 4. What the User Actually Sees

**Device Authorization Grant (GitHub & GitLab):**
1. User clicks "Connect to GitHub/GitLab."
2. Cerebrite shows a short code (e.g. `WDJB-MJHT`) and a URL.
3. User opens that URL in any browser on any device, types the code, authorizes.
4. Cerebrite, polling in background, silently picks up the token and shows "Connected."
- Cost: one manual code-copy step; user must actively go to a browser themselves; no automatic redirect back. Feels like `gh auth login`/`glab auth login`. Zero local networking or scheme registration.

**PKCE + Loopback (desktop only, practically):**
1. User clicks "Connect."
2. Cerebrite opens system browser directly to provider's authorize page.
3. User logs in/approves.
4. Browser auto-redirects to `http://127.0.0.1:<port>/callback`, captured by local listener; user returns to Cerebrite, already connected.
- Cost to user: near-zero extra steps beyond a normal "Login with GitHub" web flow — most "effortless" UX, but more fragile engineering given GitHub's fixed-port redirect-URI registration limit and Android awkwardness.

**PKCE + Custom Scheme (desktop now, Android-ready later via Tauri deep-link plugin):** similar UX to loopback but redirect target is `cerebrite://oauth/callback`, activating the app directly instead of a local HTTP listener; needs build-time scheme registration per platform.

---

## Recommendation

- **Neither provider is disqualified from OAuth** — both GitHub and GitLab have genuine secret-free (public-client) flows.
- **Device Authorization Grant is the constraint-safest choice for both providers**: zero redirect-URI/port/scheme plumbing, survives Android-possible trivially, and is a well-understood UX pattern (`gh`/`glab` CLI precedent). Costs the user one manual code-entry step.
- **PKCE (loopback or custom-scheme) is the more "effortless" UX** for desktop-now, at the cost of GitHub's fixed-port redirect-URI registration friction and generally more moving parts (local listener or deep-link registration, single-instance handling).
- **GitHub**: register as a **GitHub App** (not OAuth App) — scoped to "Contents: Read and write" only, short-lived token + secret-free refresh is a feature for background sync, and PKCE support is available if the effortless UX is chosen.
- **GitLab**: register a **non-confidential ("public") Application** with PKCE and/or device_code grant enabled; must request the broad **`api`** scope (not `write_repository`) for OAuth-token git push to work — a known GitLab limitation.
- Flag for ticket 06 (mechanism decision): if the OAuth token can serve directly as the HTTPS credential (it can, for both providers, once correctly scoped), OAuth and HTTPS-token auth are the same underlying mechanism with two ways of acquiring the credential — not a third parallel mechanism.
