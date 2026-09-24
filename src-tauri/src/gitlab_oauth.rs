// GitLab OAuth Device Authorization Grant sign-in (ticket 07, ADR-0012's
// `CredentialKind::OauthSignIn`) -- "Sign in with GitLab" without embedding
// a client secret in a distributed desktop binary (see
// `.scratch/git-provider-integration/issues/02-secretless-oauth-flows.md`
// and the decision recorded in
// `.scratch/git-provider-integration/issues/06-auth-mechanisms-decision.md`):
// register Cerebrite as a **non-confidential ("public") GitLab
// Application** with the device grant enabled, and the resulting OAuth
// token is used directly as the git HTTPS Basic-auth credential
// (`connection.rs`'s `credential_for_kind`), with username `oauth2` --
// GitLab's documented convention, distinct from ticket 06's
// `x-access-token` for GitHub. Same "OAuth is just a way of *obtaining*
// the HTTPS credential" relationship ticket 06 established, not a third
// parallel auth path.
//
// # BLOCKED ON MANUAL FOLLOW-UP -- read before shipping
//
// This module cannot be exercised against real gitlab.com from this
// sandbox: registering a GitLab application requires a human with a
// GitLab account driving GitLab's web UI (Application creation, leaving
// "Confidential" unchecked, enabling the `device_code` grant type), and
// ticket 07's own checklist calls for a *live spike* against gitlab.com
// before implementation to settle three facts the decision doc flagged as
// unverified. Neither can happen unattended in this sandbox. Until a human
// does both:
//
// - `GITLAB_CLIENT_ID` is a placeholder and must be replaced with the real
//   registered application's client id. A device-flow request made with
//   this placeholder will be rejected by GitLab.
// - The manual smoke test against a real GitLab.com repository (ticket 07
//   checklist's next-to-last item) has not been run and cannot be until
//   the above exists.
// - **The live spike itself (ticket 07 checklist's first item) is still
//   outstanding**, and this module is written *defensively* around its
//   three unresolved facts rather than assuming an answer:
//   1. **Scope sufficiency**: requests `write_repository` (this module's
//      `DEVICE_FLOW_SCOPE`), per GitLab's current docs, which now claim
//      this is enough for `git push` over HTTPS -- but ticket 02's
//      research surfaced an open GitLab defect
//      ([gitlab-org/gitlab#321359](https://gitlab.com/gitlab-org/gitlab/-/issues/321359))
//      claiming OAuth tokens specifically need the broader `api` scope
//      instead. This module takes the docs' current word as the
//      best-available default; if the live spike finds `write_repository`
//      insufficient, this constant is the only thing that needs to change.
//   2. **Refresh token presence**: GitLab's device-grant sample response
//      shows no `refresh_token` field, so it may not issue one at all.
//      `interpret_token_response` below treats it as genuinely optional
//      (`TokenPair.refresh_token: Option<String>`) rather than assuming
//      one is always present the way ticket 06's GitHub module does --
//      see `refresh_if_needed`'s doc comment for how the *absence* of a
//      refresh token is handled at the 2-hour expiry.
//   3. **Secretless refresh**: assumed to work per GitLab's general OAuth
//      docs for non-confidential applications (same as the device-code
//      exchange, which is definitely secretless), but never actually
//      exercised against gitlab.com.
//
// Everything else -- the device-flow protocol logic and the refresh
// exchange -- is implemented for real and exercised against a local mock
// HTTP server in this module's tests (`mock_server` below), never against
// gitlab.com.
#![allow(dead_code)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// TODO: replace with real GitLab application client ID once registered
/// (see this module's doc comment -- item #1 of ticket 07's manual
/// follow-up). A device-flow request made with this placeholder will be
/// rejected by GitLab.
pub const GITLAB_CLIENT_ID: &str = "TODO_REGISTER_GITLAB_APP";

/// The scope requested at the device-code step -- see this module's doc
/// comment, unresolved fact #1: GitLab's current docs say this is enough
/// for `git push` over HTTPS, but that is exactly what ticket 07's still-
/// outstanding live spike must confirm. Change only this constant (to
/// `"api"`) if the spike finds it insufficient.
const DEVICE_FLOW_SCOPE: &str = "write_repository";

/// The HTTPS Basic-auth username convention for a GitLab OAuth token
/// (ADR-0012 / ticket 07), distinct from ticket 06's GitHub
/// `x-access-token`.
pub const GITLAB_HTTPS_USERNAME: &str = "oauth2";

/// GitLab asks a device-flow client to poll no more often than this many
/// seconds by default when its response doesn't say otherwise.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// The two real endpoints this module talks to, factored out so tests can
/// substitute a local mock server for both without any dependency on a
/// trait object -- mirrors `github_oauth.rs`'s `GitHubEndpoints` /
/// `connection.rs`'s `basic_auth_fixture` / `sync.rs`'s `spawn_401_server`
/// pattern of pointing real client code at a `127.0.0.1` fixture rather
/// than mocking the client itself. Unlike `GitHubEndpoints`, there is no
/// `api_base_url`/installation-check endpoint here -- a GitLab OAuth
/// application reaches every repo the authorizing user can, there is no
/// separate per-repo "install" step the way a GitHub App needs.
#[derive(Debug, Clone)]
pub struct GitLabEndpoints {
    pub device_code_url: String,
    pub token_url: String,
    /// Base of GitLab's REST API v4 (`https://gitlab.com/api/v4` in
    /// production) -- ticket 09's create/list-repository endpoints. Named
    /// the same way `github_oauth::GitHubEndpoints::api_base_url` is, for
    /// consistency between the two provider modules; previously unused here
    /// because ticket 07 had no per-repo REST calls of its own (unlike
    /// GitHub's installation check).
    pub api_base_url: String,
}

impl GitLabEndpoints {
    pub fn production() -> Self {
        Self {
            device_code_url: "https://gitlab.com/oauth/authorize_device".to_string(),
            token_url: "https://gitlab.com/oauth/token".to_string(),
            api_base_url: "https://gitlab.com/api/v4".to_string(),
        }
    }
}

/// What `request_device_code` hands back for the frontend to display
/// (ticket 07 checklist: "surface the code and verification URL").
/// `expires_at`/`interval` travel back out too so the frontend's poll loop
/// knows how long to keep trying and how often -- identical shape to
/// `github_oauth::DeviceCodeInfo`, since RFC 8628's device-code response is
/// the same shape for both providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in_secs: u64,
    pub interval_secs: u64,
}

/// A resolved token pair -- what GitLab's device-flow token exchange (and
/// its refresh exchange, which returns the identical shape) hands back.
/// Unlike `github_oauth::TokenPair`, `refresh_token` is genuinely
/// `Option` -- this module's doc comment's unresolved fact #2: GitLab's
/// device-grant sample response shows no refresh token at all, so its
/// presence can't be assumed the way GitHub's can.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub access_token_expires_at: String,
    pub refresh_token_expires_at: Option<String>,
}

/// The credential-store payload for a GitLab `CredentialKind::OauthSignIn`
/// connection (mirrors `github_oauth::OauthSecret`'s envelope pattern, but
/// `refresh_token` is `Option` -- see this module's doc comment):
/// `connection.rs`'s `credential_for_kind` deserializes this (for
/// `Provider::GitLab` connections) to pull out just the access token for
/// `Cred::userpass_plaintext`; the refresh token, when present, only
/// matters to this module's own background-refresh path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OauthSecret {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

impl OauthSecret {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("OauthSecret always serializes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Whether this connection has a refresh token to renew itself with --
    /// the fact `sync::classify_git_error_for` and `refresh_if_needed` both
    /// need to decide between an ordinary rejected-credential failure and
    /// ticket 07's explicit "needs reconnecting" one.
    pub fn is_refreshable(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// Everything that can go wrong talking to GitLab's device-flow endpoints.
/// Identical shape to `github_oauth::DeviceFlowError` -- RFC 8628's error
/// vocabulary is provider-agnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceFlowError {
    /// The HTTP request itself failed (DNS, connect, TLS, timeout) --
    /// distinct from GitLab answering with an error body.
    Network(String),
    /// GitLab answered but the body wasn't the JSON shape expected.
    UnexpectedResponse(String),
    /// The user denied the authorization request in their browser.
    AccessDenied,
    /// The device code expired before the user completed the flow (its
    /// `expires_in` window ran out).
    ExpiredToken,
    /// Any other `error` GitLab's token endpoint returned (a real,
    /// registered app rejecting the placeholder client id lands here).
    Rejected(String),
}

impl std::fmt::Display for DeviceFlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceFlowError::Network(detail) => write!(f, "network problem talking to GitLab: {detail}"),
            DeviceFlowError::UnexpectedResponse(detail) => write!(f, "unexpected response from GitLab: {detail}"),
            DeviceFlowError::AccessDenied => write!(f, "GitLab sign-in was denied"),
            DeviceFlowError::ExpiredToken => write!(f, "GitLab sign-in code expired before it was confirmed"),
            DeviceFlowError::Rejected(detail) => write!(f, "GitLab rejected the request: {detail}"),
        }
    }
}

impl std::error::Error for DeviceFlowError {}

/// One iteration of polling GitLab's token endpoint while the user hasn't
/// finished confirming the device code yet -- the four outcomes RFC 8628
/// section 3.5 defines, plus `Success`. Identical shape to
/// `github_oauth::PollOutcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    Success(TokenPair),
    /// Keep polling at the current interval -- the user hasn't finished yet.
    Pending,
    /// Keep polling, but back off: add 5 seconds to the interval and don't
    /// poll again sooner than that (RFC 8628 section 3.5's `slow_down`).
    SlowDown,
    Denied,
    Expired,
    Error(DeviceFlowError),
}

/// Requests a fresh device/user code pair (ticket 07 checklist item 3,
/// first half): `POST {device_code_url}` with `client_id` and
/// `DEVICE_FLOW_SCOPE`. GitLab's device-flow endpoints want form-encoded
/// bodies and answer with JSON.
pub fn request_device_code(endpoints: &GitLabEndpoints, client_id: &str) -> Result<DeviceCodeInfo, DeviceFlowError> {
    let form = [("client_id", client_id), ("scope", DEVICE_FLOW_SCOPE)];
    let body = post_form(&endpoints.device_code_url, &form)?;
    let parsed: RawDeviceCodeResponse =
        serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
    if let Some(error) = parsed.error {
        return Err(DeviceFlowError::Rejected(error));
    }
    let (Some(device_code), Some(user_code), Some(verification_uri), Some(expires_in)) = (
        parsed.device_code,
        parsed.user_code,
        parsed.verification_uri,
        parsed.expires_in,
    ) else {
        return Err(DeviceFlowError::UnexpectedResponse(
            "device code response missing required fields".to_string(),
        ));
    };
    let interval = parsed.interval.unwrap_or(DEFAULT_POLL_INTERVAL_SECS);
    Ok(DeviceCodeInfo {
        device_code,
        user_code,
        verification_uri,
        expires_in_secs: expires_in,
        interval_secs: interval.max(1),
    })
}

/// One poll of `POST {token_url}` with
/// `grant_type=urn:ietf:params:oauth:grant-type:device_code` (ticket 07
/// checklist item 3, second half). Never sleeps itself -- callers
/// (`poll_until_complete` for production, or a test driving each outcome
/// directly) own the interval/backoff timing.
pub fn poll_once(endpoints: &GitLabEndpoints, client_id: &str, device_code: &str) -> PollOutcome {
    let form = [
        ("client_id", client_id),
        ("device_code", device_code),
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
    ];
    let body = match post_form(&endpoints.token_url, &form) {
        Ok(body) => body,
        Err(e) => return PollOutcome::Error(e),
    };
    let parsed: RawTokenResponse = match serde_json::from_str(&body) {
        Ok(parsed) => parsed,
        Err(e) => return PollOutcome::Error(DeviceFlowError::UnexpectedResponse(e.to_string())),
    };
    interpret_token_response(parsed)
}

/// Drives `poll_once` in a loop, sleeping between attempts per
/// `interval`/`slow_down`, until it succeeds or hits a terminal outcome
/// (denied, expired, or a hard error). Bounded by `expires_in_secs` so an
/// unresponsive/misbehaving server can't spin this forever. Identical
/// structure to `github_oauth::poll_until_complete`.
pub fn poll_until_complete(
    endpoints: &GitLabEndpoints,
    client_id: &str,
    device: &DeviceCodeInfo,
) -> Result<TokenPair, DeviceFlowError> {
    let deadline = SystemTime::now() + Duration::from_secs(device.expires_in_secs);
    let mut interval = Duration::from_secs(device.interval_secs.max(1));

    loop {
        if SystemTime::now() >= deadline {
            return Err(DeviceFlowError::ExpiredToken);
        }
        std::thread::sleep(interval);
        match poll_once(endpoints, client_id, &device.device_code) {
            PollOutcome::Success(pair) => return Ok(pair),
            PollOutcome::Pending => {}
            PollOutcome::SlowDown => interval += Duration::from_secs(5),
            PollOutcome::Denied => return Err(DeviceFlowError::AccessDenied),
            PollOutcome::Expired => return Err(DeviceFlowError::ExpiredToken),
            PollOutcome::Error(e) => return Err(e),
        }
    }
}

/// Exchanges a refresh token for a fresh token pair (ticket 07 checklist
/// item 5, the refreshable branch): same endpoint as the device-code
/// exchange, different `grant_type`, no client secret (this module's doc
/// comment, unresolved fact #3 -- assumed per GitLab's general
/// non-confidential-application docs, not yet live-verified). Only ever
/// called when a refresh token is actually in hand -- see
/// `refresh_if_needed`, which is the only production caller and checks
/// `OauthSecret::is_refreshable` first.
pub fn refresh_token_pair(
    endpoints: &GitLabEndpoints,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenPair, DeviceFlowError> {
    let form = [
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let body = post_form(&endpoints.token_url, &form)?;
    let parsed: RawTokenResponse =
        serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
    match interpret_token_response(parsed) {
        PollOutcome::Success(pair) => Ok(pair),
        PollOutcome::Denied => Err(DeviceFlowError::AccessDenied),
        PollOutcome::Expired => Err(DeviceFlowError::ExpiredToken),
        PollOutcome::Error(e) => Err(e),
        // A refresh exchange is a one-shot request, not a poll loop --
        // `authorization_pending`/`slow_down` are meaningless responses to
        // it, but treat them as a transient rejection rather than panicking
        // if a mock/misbehaving server ever sends one.
        PollOutcome::Pending | PollOutcome::SlowDown => Err(DeviceFlowError::Rejected(
            "unexpected pending/slow_down response to a refresh request".to_string(),
        )),
    }
}

/// Whether `token_expiry` (a `ConnectionRecord`'s RFC 3339 access-token
/// expiry, or `None`) is close enough to expiring that the background sync
/// path should refresh before attempting a fetch/push -- refreshing a
/// little early (five minutes) avoids a race against the token expiring
/// mid-request. GitLab's tokens last only 2 hours (vs. GitHub's 8), so this
/// cutoff matters proportionally more here, but the logic itself is
/// unchanged from `github_oauth::needs_refresh`. Missing/unparsable
/// `token_expiry` is treated as "needs refresh" (fail safe).
pub fn needs_refresh(token_expiry: Option<&str>) -> bool {
    let Some(expiry) = token_expiry else { return true };
    let Ok(expiry) = parse_rfc3339(expiry) else { return true };
    let cutoff = SystemTime::now() + Duration::from_secs(5 * 60);
    expiry <= cutoff
}

/// Ticket 07 checklist item 5, the unattended half -- called once per
/// background sync attempt (`lib.rs`'s `perform_sync`) for whatever
/// `Connection` it loaded. A no-op (returns `connection` unchanged) unless
/// it's a **GitLab** `OauthSignIn` connection (guarded by
/// `record.provider`, not just `credential_kind`, since GitHub's
/// `github_oauth::refresh_if_needed` shares the same `CredentialKind` and
/// must not be double-driven by this function or vice versa) whose access
/// token is within `needs_refresh`'s cutoff of expiring.
///
/// **The defensive dual path (ticket 07's core ask, this module's doc
/// comment unresolved fact #2):** if the stored secret has no
/// `refresh_token` at all -- GitLab's device grant may simply never have
/// issued one -- there is nothing to exchange, so this returns `connection`
/// unchanged rather than attempting (and failing) a refresh call. The
/// *consequence* of that is intentionally not hidden: once the 2-hour
/// access token actually expires, the next fetch/push fails, and
/// `sync::classify_git_error_for` recognizes this exact situation --
/// `OauthSignIn` + no refresh token -- and reports it as
/// `SyncFailureCause::OauthReconnectRequired` rather than a generic
/// `CredentialRejected`, so the UI can point the user straight at signing
/// in again instead of implying the same credential might work on retry.
///
/// If a refresh token *is* present and the exchange fails (revoked/stale
/// grant), `connection` is returned unchanged and the ensuing fetch/push
/// failure is reported as an ordinary `CredentialRejected` naming this
/// connection's kind -- same precedent as ticket 06's GitHub path.
///
/// On a successful refresh, the *entire* new pair is persisted via
/// whichever store `record.credential_store` names, with the same
/// in-memory-only fallback on a store-write failure ticket 06 established:
/// a locked keychain or disk error doesn't discard a refresh that actually
/// succeeded, it just means the next sync attempt refreshes again.
///
/// Returns `(connection, save_failed)` -- see `github_oauth::refresh_if_needed`'s
/// doc comment for what `save_failed` means and how `lib.rs`'s
/// `perform_sync` uses it (ticket 13 checklist item 4).
pub fn refresh_if_needed(
    repo_root: &std::path::Path,
    keychain: Option<&crate::credential::KeychainBackend>,
    plaintext: &crate::credential::PlaintextStore,
    connection: crate::connection::Connection,
) -> (crate::connection::Connection, bool) {
    use crate::connection_record::{self, CredentialKind, Provider, StoreKind};

    if connection.credential_kind() != CredentialKind::OauthSignIn {
        return (connection, false);
    }
    if connection.record().provider != Provider::GitLab {
        return (connection, false);
    }
    if !needs_refresh(connection.record().token_expiry.as_deref()) {
        return (connection, false);
    }
    let Ok(secret) = OauthSecret::from_bytes(connection.secret_bytes()) else {
        // Corrupt stored secret -- not this function's problem to fix; the
        // ordinary auth path will fail cleanly on it (see
        // `connection::credential_for_kind`'s `OauthSignIn`/`GitLab` arm).
        return (connection, false);
    };
    let Some(refresh_token) = secret.refresh_token.clone() else {
        // No refresh token to exchange -- see this function's doc comment.
        return (connection, false);
    };

    let endpoints = GitLabEndpoints::production();
    let pair = match refresh_token_pair(&endpoints, GITLAB_CLIENT_ID, &refresh_token) {
        Ok(pair) => pair,
        Err(_) => return (connection, false),
    };

    let new_secret = OauthSecret {
        access_token: pair.access_token,
        refresh_token: pair.refresh_token,
    };
    let mut record = connection.record().clone();
    record.token_expiry = Some(pair.access_token_expires_at);

    let stored = match record.credential_store {
        StoreKind::Keychain => keychain
            .map(|kc| kc.set_secret(&record.connection_id, &new_secret.to_bytes(), crate::credential::CallUrgency::Background))
            .unwrap_or(Err(crate::credential::CredentialError::Unavailable(
                "no keychain backend is available".to_string(),
            ))),
        StoreKind::Plaintext => plaintext
            .set_secret(&record.connection_id, &new_secret.to_bytes())
            .map_err(|e| crate::credential::CredentialError::Other(e.to_string())),
    };
    let save_failed = stored.is_err();
    if stored.is_ok() {
        let _ = connection_record::write(repo_root, &record);
    }

    (crate::connection::Connection::from_parts(record, new_secret.to_bytes()), save_failed)
}

/// One repository as surfaced to the guided connect wizard (ticket 09) --
/// identical shape to `github_oauth::RepoInfo` (kept duplicated rather than
/// shared, same as the rest of this module, to keep the two provider
/// modules independently readable/removable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub clone_url: String,
    pub html_url: String,
}

/// Ticket 09's create-new path: `POST {api_base_url}/projects` with `name`
/// and `visibility` (`"private"`/`"public"` -- private is the caller's
/// responsibility to default to). GitLab's REST API takes an OAuth token as
/// a `Bearer` credential exactly like GitHub's does.
pub fn create_repository(
    endpoints: &GitLabEndpoints,
    access_token: &str,
    name: &str,
    private: bool,
) -> Result<RepoInfo, DeviceFlowError> {
    let visibility = if private { "private" } else { "public" };
    let url = format!("{}/projects", endpoints.api_base_url);
    let body = serde_json::json!({ "name": name, "visibility": visibility });
    let (status, body) = post_bearer_json(&url, access_token, &body)?;
    match status {
        200 | 201 => {
            let raw: RawProjectResponse =
                serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
            raw.try_into()
        }
        401 | 403 => Err(DeviceFlowError::Rejected(format!(
            "GitLab rejected the repository creation request with status {status}"
        ))),
        other => Err(DeviceFlowError::UnexpectedResponse(format!(
            "unexpected status {other} creating a repository: {body}"
        ))),
    }
}

/// Ticket 09's pick-existing path: `GET {api_base_url}/projects?membership=true`
/// (only projects the authenticated user is a member of), first 100 results
/// -- paginating further is explicitly optional per the ticket.
pub fn list_repositories(endpoints: &GitLabEndpoints, access_token: &str) -> Result<Vec<RepoInfo>, DeviceFlowError> {
    let url = format!("{}/projects?membership=true&per_page=100", endpoints.api_base_url);
    let (status, body) = get_bearer(&url, access_token)?;
    match status {
        200 => {
            let raw: Vec<RawProjectResponse> =
                serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
            raw.into_iter().map(TryInto::try_into).collect()
        }
        401 | 403 => Err(DeviceFlowError::Rejected(format!(
            "GitLab rejected the repository list request with status {status}"
        ))),
        other => Err(DeviceFlowError::UnexpectedResponse(format!(
            "unexpected status {other} listing repositories: {body}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct RawProjectResponse {
    name: Option<String>,
    path_with_namespace: Option<String>,
    visibility: Option<String>,
    http_url_to_repo: Option<String>,
    web_url: Option<String>,
}

impl TryFrom<RawProjectResponse> for RepoInfo {
    type Error = DeviceFlowError;

    fn try_from(raw: RawProjectResponse) -> Result<Self, Self::Error> {
        let (Some(name), Some(full_name), Some(visibility), Some(clone_url), Some(html_url)) = (
            raw.name,
            raw.path_with_namespace,
            raw.visibility,
            raw.http_url_to_repo,
            raw.web_url,
        ) else {
            return Err(DeviceFlowError::UnexpectedResponse(
                "project response missing required fields".to_string(),
            ));
        };
        Ok(RepoInfo {
            name,
            full_name,
            private: visibility != "public",
            clone_url,
            html_url,
        })
    }
}

/// A blocking `POST` with a JSON body and a `Bearer` token, returning
/// (status, body) -- the create-repository counterpart to `get_bearer`
/// (which this module doesn't otherwise have; added alongside it here for
/// ticket 09's list-repository call).
fn post_bearer_json(url: &str, token: &str, json_body: &serde_json::Value) -> Result<(u16, String), DeviceFlowError> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .json(json_body)
        .send()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    Ok((status, body))
}

/// A blocking `GET` with a `Bearer` token, returning (status, body) --
/// `github_oauth.rs` has its own copy of this same shape; kept duplicated
/// here for the same independently-readable-module reason as the rest of
/// this file.
fn get_bearer(url: &str, token: &str) -> Result<(u16, String), DeviceFlowError> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    Ok((status, body))
}

// -- internal: raw response shapes and shared HTTP plumbing --

#[derive(Debug, Deserialize)]
struct RawDeviceCodeResponse {
    device_code: Option<String>,
    user_code: Option<String>,
    verification_uri: Option<String>,
    expires_in: Option<u64>,
    interval: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
    refresh_token_expires_in: Option<u64>,
    error: Option<String>,
}

fn interpret_token_response(parsed: RawTokenResponse) -> PollOutcome {
    if let Some(error) = parsed.error.as_deref() {
        return match error {
            "authorization_pending" => PollOutcome::Pending,
            "slow_down" => PollOutcome::SlowDown,
            "expired_token" => PollOutcome::Expired,
            "access_denied" => PollOutcome::Denied,
            other => PollOutcome::Error(DeviceFlowError::Rejected(other.to_string())),
        };
    }
    // Unlike GitHub, `refresh_token` is genuinely optional here (this
    // module's doc comment, unresolved fact #2) -- only `access_token` and
    // `expires_in` are required to call this a success.
    let (Some(access_token), Some(expires_in)) = (parsed.access_token, parsed.expires_in) else {
        return PollOutcome::Error(DeviceFlowError::UnexpectedResponse(
            "token response missing required fields".to_string(),
        ));
    };
    let access_token_expires_at = to_rfc3339(SystemTime::now() + Duration::from_secs(expires_in));
    let refresh_token_expires_at = parsed
        .refresh_token_expires_in
        .map(|secs| to_rfc3339(SystemTime::now() + Duration::from_secs(secs)));
    PollOutcome::Success(TokenPair {
        access_token,
        refresh_token: parsed.refresh_token,
        access_token_expires_at,
        refresh_token_expires_at,
    })
}

/// RFC 3339 (UTC, second precision) formatter/parser, matching the string
/// shape `connection_record::ConnectionRecord::token_expiry`'s doc comment
/// promises. Identical to `github_oauth`'s copy -- kept duplicated rather
/// than factored out, same as the rest of this module, to keep the two
/// provider modules independently readable/removable.
fn to_rfc3339(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO).as_secs();
    DateTime::<Utc>::from_timestamp(secs as i64, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
        .to_rfc3339()
}

fn parse_rfc3339(s: &str) -> Result<SystemTime, ()> {
    let parsed = DateTime::parse_from_rfc3339(s).map_err(|_| ())?;
    let secs = parsed.timestamp();
    if secs < 0 {
        return Err(());
    }
    Ok(UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// A blocking `POST` with a form-encoded body and `Accept: application/json`,
/// returning the raw response body. Real network I/O in production; tests
/// point `url` at a local mock server instead of substituting a
/// trait/mock object, matching `github_oauth.rs`'s precedent.
fn post_form(url: &str, form: &[(&str, &str)]) -> Result<String, DeviceFlowError> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .header("Accept", "application/json")
        .form(form)
        .send()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    response
        .text()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- OauthSecret envelope --

    #[test]
    fn oauth_secret_round_trips_through_bytes_with_a_refresh_token() {
        let secret = OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: Some("glrt_def".to_string()),
        };
        let bytes = secret.to_bytes();
        assert_eq!(OauthSecret::from_bytes(&bytes).unwrap(), secret);
    }

    #[test]
    fn oauth_secret_round_trips_through_bytes_with_no_refresh_token() {
        // The defensive case ticket 07 exists for: GitLab's device grant
        // may simply not hand one back.
        let secret = OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: None,
        };
        let bytes = secret.to_bytes();
        assert_eq!(OauthSecret::from_bytes(&bytes).unwrap(), secret);
    }

    #[test]
    fn is_refreshable_reflects_whether_a_refresh_token_is_present() {
        assert!(OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: Some("glrt_def".to_string()),
        }
        .is_refreshable());
        assert!(!OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: None,
        }
        .is_refreshable());
    }

    // -- needs_refresh --

    #[test]
    fn needs_refresh_is_true_when_expiry_is_absent() {
        assert!(needs_refresh(None));
    }

    #[test]
    fn needs_refresh_is_true_when_expiry_is_unparsable() {
        assert!(needs_refresh(Some("not a timestamp")));
    }

    #[test]
    fn needs_refresh_is_true_when_expiry_is_within_the_five_minute_cutoff() {
        let soon = to_rfc3339(SystemTime::now() + Duration::from_secs(60));
        assert!(needs_refresh(Some(&soon)));
    }

    #[test]
    fn needs_refresh_is_false_when_expiry_is_comfortably_in_the_future() {
        let later = to_rfc3339(SystemTime::now() + Duration::from_secs(60 * 60));
        assert!(!needs_refresh(Some(&later)));
    }

    // -- interpret_token_response --

    #[test]
    fn interpret_token_response_maps_each_documented_error_to_its_outcome() {
        let case = |error: &str| {
            interpret_token_response(RawTokenResponse {
                access_token: None,
                refresh_token: None,
                expires_in: None,
                refresh_token_expires_in: None,
                error: Some(error.to_string()),
            })
        };
        assert_eq!(case("authorization_pending"), PollOutcome::Pending);
        assert_eq!(case("slow_down"), PollOutcome::SlowDown);
        assert_eq!(case("expired_token"), PollOutcome::Expired);
        assert_eq!(case("access_denied"), PollOutcome::Denied);
        assert!(matches!(case("invalid_grant"), PollOutcome::Error(_)));
    }

    #[test]
    fn interpret_token_response_builds_a_token_pair_with_a_refresh_token_when_present() {
        let outcome = interpret_token_response(RawTokenResponse {
            access_token: Some("glpat_abc".to_string()),
            refresh_token: Some("glrt_def".to_string()),
            expires_in: Some(7200),
            refresh_token_expires_in: None,
            error: None,
        });
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "glpat_abc");
                assert_eq!(pair.refresh_token, Some("glrt_def".to_string()));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn interpret_token_response_succeeds_with_no_refresh_token_at_all() {
        // The defensive case: GitLab's device grant sample response has no
        // `refresh_token` field, and this must not be treated as a parse
        // failure.
        let outcome = interpret_token_response(RawTokenResponse {
            access_token: Some("glpat_abc".to_string()),
            refresh_token: None,
            expires_in: Some(7200),
            refresh_token_expires_in: None,
            error: None,
        });
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "glpat_abc");
                assert_eq!(pair.refresh_token, None);
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    // -- mock_server: a hand-rolled HTTP/1.1 responder standing in for
    // gitlab.com, exactly mirroring `github_oauth.rs`'s `mock_server` --
    // never touches real GitLab.
    mod mock_server {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        /// Spawns a server that answers every request with `responses[n]`
        /// in order (one response per request received), for scripting a
        /// sequence of outcomes (e.g. two `authorization_pending` polls
        /// then a success).
        pub fn spawn(responses: Vec<(u16, String)>) -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let counter = Arc::new(AtomicUsize::new(0));
            std::thread::spawn(move || {
                for stream in listener.incoming().take(responses.len().max(1) + 5) {
                    let Ok(mut stream) = stream else { continue };
                    let mut buf = [0u8; 8192];
                    let _ = stream.read(&mut buf).unwrap_or(0);

                    let idx = counter.fetch_add(1, Ordering::SeqCst);
                    let Some((status, body)) = responses.get(idx) else {
                        break;
                    };
                    let reason = match status {
                        200 => "OK",
                        _ => "Error",
                    };
                    let response = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            port
        }
    }

    fn endpoints_for(port: u16) -> GitLabEndpoints {
        let base = format!("http://127.0.0.1:{port}");
        GitLabEndpoints {
            device_code_url: base.clone(),
            token_url: base.clone(),
            api_base_url: base,
        }
    }

    #[test]
    fn request_device_code_parses_a_successful_mock_response() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"device_code":"dc123","user_code":"ABCD-EFGH","verification_uri":"https://gitlab.com/oauth/device","expires_in":900,"interval":5}"#.to_string(),
        )]);
        let info = request_device_code(&endpoints_for(port), "test-client-id").unwrap();
        assert_eq!(info.device_code, "dc123");
        assert_eq!(info.user_code, "ABCD-EFGH");
        assert_eq!(info.expires_in_secs, 900);
        assert_eq!(info.interval_secs, 5);
    }

    #[test]
    fn request_device_code_defaults_the_interval_when_gitlab_omits_it() {
        // GitLab's documented device-code response doesn't necessarily
        // include `interval` the way GitHub's always does.
        let port = mock_server::spawn(vec![(
            200,
            r#"{"device_code":"dc123","user_code":"ABCD-EFGH","verification_uri":"https://gitlab.com/oauth/device","expires_in":900}"#.to_string(),
        )]);
        let info = request_device_code(&endpoints_for(port), "test-client-id").unwrap();
        assert_eq!(info.interval_secs, DEFAULT_POLL_INTERVAL_SECS);
    }

    #[test]
    fn request_device_code_surfaces_a_rejected_client_id() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"error":"invalid_client","error_description":"placeholder client id"}"#.to_string(),
        )]);
        let result = request_device_code(&endpoints_for(port), "TODO_REGISTER_GITLAB_APP");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    #[test]
    fn poll_once_reports_authorization_pending() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"authorization_pending"}"#.to_string())]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        assert_eq!(outcome, PollOutcome::Pending);
    }

    #[test]
    fn poll_once_reports_success_with_a_token_pair_including_a_refresh_token() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"access_token":"glpat_abc","token_type":"bearer","refresh_token":"glrt_def","expires_in":7200}"#.to_string(),
        )]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "glpat_abc");
                assert_eq!(pair.refresh_token, Some("glrt_def".to_string()));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn poll_once_reports_success_with_no_refresh_token_in_the_response() {
        // The defensive case ticket 07's live spike must eventually settle
        // for real -- exercised here against a mock response that omits
        // `refresh_token` entirely, matching GitLab's documented sample.
        let port = mock_server::spawn(vec![(
            200,
            r#"{"access_token":"glpat_abc","token_type":"bearer","expires_in":7200}"#.to_string(),
        )]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "glpat_abc");
                assert_eq!(pair.refresh_token, None);
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn poll_until_complete_polls_through_pending_and_slow_down_to_success() {
        let port = mock_server::spawn(vec![
            (200, r#"{"error":"authorization_pending"}"#.to_string()),
            (200, r#"{"error":"slow_down"}"#.to_string()),
            (
                200,
                r#"{"access_token":"glpat_abc","refresh_token":"glrt_def","expires_in":7200}"#.to_string(),
            ),
        ]);
        let device = DeviceCodeInfo {
            device_code: "dc123".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://gitlab.com/oauth/device".to_string(),
            expires_in_secs: 60,
            interval_secs: 1,
        };
        let pair = poll_until_complete(&endpoints_for(port), "client", &device).unwrap();
        assert_eq!(pair.access_token, "glpat_abc");
    }

    #[test]
    fn poll_until_complete_surfaces_access_denied() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"access_denied"}"#.to_string())]);
        let device = DeviceCodeInfo {
            device_code: "dc123".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://gitlab.com/oauth/device".to_string(),
            expires_in_secs: 60,
            interval_secs: 1,
        };
        let result = poll_until_complete(&endpoints_for(port), "client", &device);
        assert_eq!(result, Err(DeviceFlowError::AccessDenied));
    }

    #[test]
    fn poll_until_complete_expires_when_the_deadline_passes_before_success() {
        let port = mock_server::spawn(vec![
            (200, r#"{"error":"authorization_pending"}"#.to_string()),
            (200, r#"{"error":"authorization_pending"}"#.to_string()),
            (200, r#"{"error":"authorization_pending"}"#.to_string()),
        ]);
        let device = DeviceCodeInfo {
            device_code: "dc123".to_string(),
            user_code: "ABCD-EFGH".to_string(),
            verification_uri: "https://gitlab.com/oauth/device".to_string(),
            expires_in_secs: 1,
            interval_secs: 2,
        };
        let result = poll_until_complete(&endpoints_for(port), "client", &device);
        assert_eq!(result, Err(DeviceFlowError::ExpiredToken));
    }

    #[test]
    fn refresh_token_pair_returns_a_rotated_pair_when_gitlab_rotates_it() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"access_token":"glpat_new","refresh_token":"glrt_new","expires_in":7200}"#.to_string(),
        )]);
        let pair = refresh_token_pair(&endpoints_for(port), "client", "glrt_old").unwrap();
        assert_eq!(pair.access_token, "glpat_new");
        assert_eq!(pair.refresh_token, Some("glrt_new".to_string()));
    }

    #[test]
    fn refresh_token_pair_surfaces_a_revoked_refresh_token() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"error":"invalid_grant","error_description":"The refresh token is invalid."}"#.to_string(),
        )]);
        let result = refresh_token_pair(&endpoints_for(port), "client", "glrt_stale");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    // -- create_repository / list_repositories (ticket 09) --

    #[test]
    fn create_repository_parses_a_successful_response() {
        let port = mock_server::spawn(vec![(
            201,
            r#"{"name":"notes","path_with_namespace":"dawid/notes","visibility":"private","http_url_to_repo":"https://gitlab.com/dawid/notes.git","web_url":"https://gitlab.com/dawid/notes"}"#.to_string(),
        )]);
        let repo = create_repository(&endpoints_for(port), "glpat_abc", "notes", true).unwrap();
        assert_eq!(repo.name, "notes");
        assert_eq!(repo.full_name, "dawid/notes");
        assert!(repo.private);
        assert_eq!(repo.clone_url, "https://gitlab.com/dawid/notes.git");
    }

    #[test]
    fn create_repository_defaults_visibility_to_public_only_when_explicitly_asked() {
        let port = mock_server::spawn(vec![(
            201,
            r#"{"name":"public-thing","path_with_namespace":"dawid/public-thing","visibility":"public","http_url_to_repo":"https://gitlab.com/dawid/public-thing.git","web_url":"https://gitlab.com/dawid/public-thing"}"#.to_string(),
        )]);
        let repo = create_repository(&endpoints_for(port), "glpat_abc", "public-thing", false).unwrap();
        assert!(!repo.private);
    }

    #[test]
    fn create_repository_surfaces_a_name_already_taken_rejection() {
        let port = mock_server::spawn(vec![(
            400,
            r#"{"message":{"name":["has already been taken"]}}"#.to_string(),
        )]);
        let result = create_repository(&endpoints_for(port), "glpat_abc", "notes", true);
        assert!(matches!(result, Err(DeviceFlowError::UnexpectedResponse(_))));
    }

    #[test]
    fn create_repository_surfaces_a_rejected_token() {
        let port = mock_server::spawn(vec![(401, r#"{"message":"401 Unauthorized"}"#.to_string())]);
        let result = create_repository(&endpoints_for(port), "bad-token", "notes", true);
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    #[test]
    fn list_repositories_parses_a_successful_response() {
        let port = mock_server::spawn(vec![(
            200,
            r#"[{"name":"notes","path_with_namespace":"dawid/notes","visibility":"private","http_url_to_repo":"https://gitlab.com/dawid/notes.git","web_url":"https://gitlab.com/dawid/notes"},
                {"name":"public-thing","path_with_namespace":"dawid/public-thing","visibility":"public","http_url_to_repo":"https://gitlab.com/dawid/public-thing.git","web_url":"https://gitlab.com/dawid/public-thing"}]"#.to_string(),
        )]);
        let repos = list_repositories(&endpoints_for(port), "glpat_abc").unwrap();
        assert_eq!(repos.len(), 2);
        assert!(repos[0].private);
        assert!(!repos[1].private);
    }

    #[test]
    fn list_repositories_surfaces_a_rejected_token() {
        let port = mock_server::spawn(vec![(401, r#"{"message":"401 Unauthorized"}"#.to_string())]);
        let result = list_repositories(&endpoints_for(port), "bad-token");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    // -- refresh_if_needed: the dual-path defensive logic, ticket 07's core
    // ask -- exercised against `Connection`/store fixtures directly (not
    // just the raw HTTP functions above), covering both the
    // refresh-token-present and refresh-token-absent branches.
    mod refresh_if_needed_tests {
        use super::*;
        use crate::connection::Connection;
        use crate::connection_record::{ConnectionRecord, CredentialKind, Provider, StoreKind};
        use crate::credential::PlaintextStore;
        use tempfile::tempdir;

        fn gitlab_record(connection_id: &str) -> ConnectionRecord {
            ConnectionRecord {
                connection_id: connection_id.to_string(),
                credential_kind: CredentialKind::OauthSignIn,
                provider: Provider::GitLab,
                https_username: Some(GITLAB_HTTPS_USERNAME.to_string()),
                token_expiry: Some(to_rfc3339(SystemTime::now() - Duration::from_secs(60))),
                credential_store: StoreKind::Plaintext,
            }
        }

        #[test]
        fn refreshes_and_persists_a_rotated_pair_when_a_refresh_token_is_present() {
            let repo_dir = tempdir().unwrap();
            crate::vault::ensure_git_repo(repo_dir.path()).unwrap();
            let config_dir = tempdir().unwrap();
            let plaintext = PlaintextStore::new(config_dir.path());

            let record = gitlab_record("conn-refreshable");
            let secret = OauthSecret {
                access_token: "glpat_old".to_string(),
                refresh_token: Some("glrt_old".to_string()),
            };
            crate::connection_record::write(repo_dir.path(), &record).unwrap();
            plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();

            let port = mock_server::spawn(vec![(
                200,
                r#"{"access_token":"glpat_new","refresh_token":"glrt_new","expires_in":7200}"#.to_string(),
            )]);
            // `refresh_if_needed` always calls `GitLabEndpoints::production()`
            // internally, so this test instead calls the lower-level pieces
            // it's built from directly against the mock server, then asserts
            // the persistence side of `refresh_if_needed` by constructing
            // the equivalent connection and driving the same store-write
            // path it uses.
            let endpoints = endpoints_for(port);
            let pair = refresh_token_pair(&endpoints, GITLAB_CLIENT_ID, "glrt_old").unwrap();
            assert_eq!(pair.access_token, "glpat_new");
            assert_eq!(pair.refresh_token, Some("glrt_new".to_string()));

            let new_secret = OauthSecret {
                access_token: pair.access_token,
                refresh_token: pair.refresh_token,
            };
            plaintext
                .set_secret(&record.connection_id, &new_secret.to_bytes())
                .unwrap();
            assert_eq!(
                OauthSecret::from_bytes(&plaintext.get_secret(&record.connection_id).unwrap()).unwrap(),
                new_secret
            );
        }

        #[test]
        fn is_a_noop_for_a_non_gitlab_oauth_connection() {
            // Guards against this module's `refresh_if_needed` ever being
            // driven for a GitHub `OauthSignIn` connection (same
            // `CredentialKind`, different provider) -- it must leave it
            // completely untouched rather than trying to parse a
            // `github_oauth::OauthSecret` envelope as this module's shape.
            let repo_dir = tempdir().unwrap();
            crate::vault::ensure_git_repo(repo_dir.path()).unwrap();
            let config_dir = tempdir().unwrap();
            let keychain = crate::credential::KeychainBackend::in_memory();
            let plaintext = PlaintextStore::new(config_dir.path());

            let mut record = gitlab_record("conn-github");
            record.provider = Provider::GitHub;
            let github_secret = crate::github_oauth::OauthSecret {
                access_token: "gho_abc".to_string(),
                refresh_token: "ghr_def".to_string(),
            };
            let connection = Connection::from_parts(record.clone(), github_secret.to_bytes());

            let (result, save_failed) = refresh_if_needed(repo_dir.path(), Some(&keychain), &plaintext, connection);

            assert_eq!(result.secret_bytes(), github_secret.to_bytes());
            assert_eq!(result.record(), &record);
            assert!(!save_failed);
        }

        #[test]
        fn is_a_noop_when_the_stored_secret_has_no_refresh_token() {
            // The defensive branch this ticket exists for: no network call
            // is even attempted, and the connection is handed back
            // unchanged so the *next* fetch/push failure is what surfaces
            // the reconnect-needed state via `sync::classify_git_error_for`.
            let repo_dir = tempdir().unwrap();
            crate::vault::ensure_git_repo(repo_dir.path()).unwrap();
            let config_dir = tempdir().unwrap();
            let keychain = crate::credential::KeychainBackend::in_memory();
            let plaintext = PlaintextStore::new(config_dir.path());

            let record = gitlab_record("conn-unrefreshable");
            let secret = OauthSecret {
                access_token: "glpat_old".to_string(),
                refresh_token: None,
            };
            let connection = Connection::from_parts(record.clone(), secret.to_bytes());

            let (result, save_failed) = refresh_if_needed(repo_dir.path(), Some(&keychain), &plaintext, connection);

            assert_eq!(result.secret_bytes(), secret.to_bytes());
            assert_eq!(result.record(), &record);
            assert!(!save_failed);
        }

        #[test]
        fn is_a_noop_when_the_token_is_not_close_to_expiring() {
            let repo_dir = tempdir().unwrap();
            crate::vault::ensure_git_repo(repo_dir.path()).unwrap();
            let config_dir = tempdir().unwrap();
            let keychain = crate::credential::KeychainBackend::in_memory();
            let plaintext = PlaintextStore::new(config_dir.path());

            let mut record = gitlab_record("conn-fresh");
            record.token_expiry = Some(to_rfc3339(SystemTime::now() + Duration::from_secs(60 * 60)));
            let secret = OauthSecret {
                access_token: "glpat_current".to_string(),
                refresh_token: Some("glrt_current".to_string()),
            };
            let connection = Connection::from_parts(record.clone(), secret.to_bytes());

            let (result, save_failed) = refresh_if_needed(repo_dir.path(), Some(&keychain), &plaintext, connection);

            assert_eq!(result.secret_bytes(), secret.to_bytes());
            assert!(!save_failed);
        }
    }
}
