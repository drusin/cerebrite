// GitHub OAuth Device Authorization Grant sign-in (ticket 06, ADR-0012's
// `CredentialKind::OauthSignIn`) -- "Sign in with GitHub" without embedding
// a client secret in a distributed desktop binary (see
// `.scratch/git-provider-integration/issues/02-secretless-oauth-flows.md`):
// register Cerebrite as a **GitHub App** (not an OAuth App), enable Device
// Flow on it, and the resulting user-to-server token is used directly as
// the git HTTPS Basic-auth credential (`connection.rs`'s
// `credential_for_kind`), same mechanism ticket 04's access token uses --
// OAuth is just a different way of *obtaining* that token, not a third
// parallel auth path.
//
// # BLOCKED ON MANUAL FOLLOW-UP -- read before shipping
//
// This module cannot be exercised against real GitHub from this sandbox:
// registering a GitHub App requires a human with a GitHub account driving
// GitHub's web UI (App creation, Device Flow opt-in, "Contents: Read and
// write" permission, generating the app slug). Until that happens:
//
// - `GITHUB_CLIENT_ID` is a placeholder and must be replaced with the real
//   registered app's client id.
// - `GITHUB_APP_SLUG` is a placeholder and must be replaced with the real
//   app's slug (used to build the installation URL).
// - The manual smoke test against a real GitHub repository (ticket 06
//   checklist's last item) has not been run and cannot be until the above
//   exists.
//
// Everything else -- the device-flow protocol logic, the refresh exchange,
// and the installation check -- is implemented for real and exercised
// against a local mock HTTP server in this module's tests (`mock_server`
// below), never against api.github.com/github.com.
#![allow(dead_code)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// TODO: replace with the real GitHub App's client id once it's registered
/// (see this module's doc comment -- item #1 of ticket 06's manual
/// follow-up). A device-flow request made with this placeholder will be
/// rejected by GitHub with `error=incorrect_client_credentials`.
pub const GITHUB_CLIENT_ID: &str = "TODO_REGISTER_GITHUB_APP";

/// TODO: replace with the real GitHub App's slug once registered -- used to
/// build the "install this app on your repo" URL
/// (`https://github.com/apps/<slug>/installations/new`).
pub const GITHUB_APP_SLUG: &str = "TODO_REGISTER_GITHUB_APP";

/// GitHub asks a device-flow client to poll no more often than this many
/// seconds by default (`interval` in its response normally says the same,
/// but this is the spec's documented floor).
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

/// The three real endpoints this module talks to, factored out so tests can
/// substitute a local mock server for all three without any dependency on a
/// trait object -- mirrors `connection.rs`'s `basic_auth_fixture` /
/// `sync.rs`'s `spawn_401_server` pattern of pointing real client code at a
/// `127.0.0.1` fixture rather than mocking the client itself.
#[derive(Debug, Clone)]
pub struct GitHubEndpoints {
    pub device_code_url: String,
    pub token_url: String,
    /// Base of the REST API (`https://api.github.com` in production) --
    /// used for the post-auth installation check.
    pub api_base_url: String,
}

impl GitHubEndpoints {
    pub fn production() -> Self {
        Self {
            device_code_url: "https://github.com/login/device/code".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            api_base_url: "https://api.github.com".to_string(),
        }
    }
}

/// What `request_device_code` hands back for the frontend to display
/// (ticket 06 checklist: "surface the code and verification URL to the
/// user"). `expires_at`/`interval` travel back out too so the frontend's
/// poll loop knows how long to keep trying and how often.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in_secs: u64,
    pub interval_secs: u64,
}

/// A resolved token pair -- what GitHub's device-flow token exchange (and
/// its refresh exchange, which returns the identical shape) hands back.
/// `access_token_expires_at`/`refresh_token_expires_at` are RFC 3339
/// timestamps computed from the `expires_in`/`refresh_token_expires_in`
/// second counts GitHub actually returns, so downstream code (this module's
/// `needs_refresh`, `connection_record::ConnectionRecord::token_expiry`)
/// never has to re-derive "now" from a raw duration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_token_expires_at: String,
    pub refresh_token_expires_at: Option<String>,
}

/// The credential-store payload for `CredentialKind::OauthSignIn` (mirrors
/// `ssh_key::SshSecret`'s envelope pattern exactly): both halves of the
/// token pair, serialized as JSON and stored as opaque bytes via ticket
/// 02's `KeychainBackend`/`PlaintextStore`. `connection.rs`'s
/// `credential_for_kind` deserializes this to pull out just the access
/// token for `Cred::userpass_plaintext`; the refresh token only matters to
/// this module's own background-refresh path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OauthSecret {
    pub access_token: String,
    pub refresh_token: String,
}

impl OauthSecret {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("OauthSecret always serializes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// Everything that can go wrong talking to GitHub's device-flow or
/// installation-check endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceFlowError {
    /// The HTTP request itself failed (DNS, connect, TLS, timeout) --
    /// distinct from GitHub answering with an error body.
    Network(String),
    /// GitHub answered but the body wasn't the JSON shape expected.
    UnexpectedResponse(String),
    /// The user denied the authorization request in their browser.
    AccessDenied,
    /// The device code expired before the user completed the flow (its
    /// `expires_in` window ran out).
    ExpiredToken,
    /// Any other `error` GitHub's token endpoint returned (a real,
    /// registered app rejecting the placeholder client id lands here as
    /// `incorrect_client_credentials`, for example).
    Rejected(String),
}

impl std::fmt::Display for DeviceFlowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceFlowError::Network(detail) => write!(f, "network problem talking to GitHub: {detail}"),
            DeviceFlowError::UnexpectedResponse(detail) => write!(f, "unexpected response from GitHub: {detail}"),
            DeviceFlowError::AccessDenied => write!(f, "GitHub sign-in was denied"),
            DeviceFlowError::ExpiredToken => write!(f, "GitHub sign-in code expired before it was confirmed"),
            DeviceFlowError::Rejected(detail) => write!(f, "GitHub rejected the request: {detail}"),
        }
    }
}

impl std::error::Error for DeviceFlowError {}

/// One iteration of polling GitHub's token endpoint while the user hasn't
/// finished confirming the device code yet -- the four outcomes the device
/// flow spec defines (RFC 8628 section 3.5), plus `Success`.
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

/// Requests a fresh device/user code pair (ticket 06 checklist item 2,
/// first half): `POST {device_code_url}` with `client_id` (and the repo
/// scope this app needs). GitHub's device-flow endpoints want form-encoded
/// bodies and answer with JSON only if asked via `Accept`.
pub fn request_device_code(endpoints: &GitHubEndpoints, client_id: &str) -> Result<DeviceCodeInfo, DeviceFlowError> {
    let form = [("client_id", client_id), ("scope", "repo")];
    let body = post_form(&endpoints.device_code_url, &form)?;
    let parsed: RawDeviceCodeResponse =
        serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
    if let Some(error) = parsed.error {
        return Err(DeviceFlowError::Rejected(error));
    }
    let (Some(device_code), Some(user_code), Some(verification_uri), Some(expires_in), Some(interval)) = (
        parsed.device_code,
        parsed.user_code,
        parsed.verification_uri,
        parsed.expires_in,
        parsed.interval,
    ) else {
        return Err(DeviceFlowError::UnexpectedResponse(
            "device code response missing required fields".to_string(),
        ));
    };
    Ok(DeviceCodeInfo {
        device_code,
        user_code,
        verification_uri,
        expires_in_secs: expires_in,
        interval_secs: interval.max(1),
    })
}

/// One poll of `POST {token_url}` with
/// `grant_type=urn:ietf:params:oauth:grant-type:device_code` (ticket 06
/// checklist item 2, second half). Never sleeps itself -- callers
/// (`poll_until_complete` for production, or a test driving each outcome
/// directly) own the interval/backoff timing, since RFC 8628's
/// `slow_down`/`interval` handling needs a caller-visible loop rather than
/// something buried in here.
pub fn poll_once(endpoints: &GitHubEndpoints, client_id: &str, device_code: &str) -> PollOutcome {
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
/// (denied, expired, or a hard error) -- the shape the production caller
/// (the frontend's device-flow command in `lib.rs`) actually wants, rather
/// than reimplementing this loop at every call site. Bounded by
/// `expires_in_secs` so an unresponsive/misbehaving server can't spin this
/// forever.
pub fn poll_until_complete(
    endpoints: &GitHubEndpoints,
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

/// Exchanges a refresh token for a fresh token pair (ticket 06 checklist
/// item 5): GitHub Apps' device flow needs no client secret for this either
/// -- same endpoint as the device-code exchange, different `grant_type`.
/// The refresh token itself rotates on every use (GitHub issues a new one
/// in the same response), so the caller must persist the *entire* returned
/// pair, not just the access token -- `lib.rs`'s background refresh path
/// does this via ticket 02's store.
pub fn refresh_token_pair(
    endpoints: &GitHubEndpoints,
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
/// mid-request. Missing/unparsable `token_expiry` is treated as "needs
/// refresh" (fail safe toward refreshing rather than silently sending a
/// stale/expired token).
pub fn needs_refresh(token_expiry: Option<&str>) -> bool {
    let Some(expiry) = token_expiry else { return true };
    let Ok(expiry) = parse_rfc3339(expiry) else { return true };
    let cutoff = SystemTime::now() + Duration::from_secs(5 * 60);
    expiry <= cutoff
}

/// Whether the GitHub App is installed on `owner/repo` (ticket 06 checklist
/// item 3): `GET {api_base_url}/repos/{owner}/{repo}/installation` with the
/// user-to-server token as a Bearer credential. A 404 means "not installed
/// on this repo" (or the token can't see the installation, which is
/// functionally the same thing from this app's point of view: no
/// credential this token grants reaches the repo), in which case the
/// caller is handed the app's installation URL to send the user to.
pub fn check_installation(
    endpoints: &GitHubEndpoints,
    access_token: &str,
    owner: &str,
    repo: &str,
) -> Result<InstallationStatus, DeviceFlowError> {
    let url = format!("{}/repos/{owner}/{repo}/installation", endpoints.api_base_url);
    let (status, _body) = get_bearer(&url, access_token)?;
    match status {
        200 => Ok(InstallationStatus::Installed),
        404 => Ok(InstallationStatus::NotInstalled {
            install_url: installation_url(),
        }),
        401 | 403 => Err(DeviceFlowError::Rejected(format!(
            "GitHub rejected the installation check with status {status}"
        ))),
        other => Err(DeviceFlowError::UnexpectedResponse(format!(
            "unexpected status {other} checking installation"
        ))),
    }
}

/// Ticket 06 checklist item 5, the unattended half: called once per
/// background sync attempt (`lib.rs`'s `perform_sync`) for whatever
/// `Connection` it loaded. A no-op (returns `connection` unchanged) unless
/// it's an `OauthSignIn` connection whose access token is within
/// `needs_refresh`'s cutoff of expiring.
///
/// On a successful refresh, GitHub rotates the refresh token too -- the
/// *entire* new pair is persisted via whichever store `record.credential_store`
/// names, mirroring ticket 02's "couldn't save your refresh" fallback: if
/// persisting fails (locked keychain, disk error), the freshly refreshed
/// pair is still used in memory for *this* sync attempt rather than
/// discarded -- an unrelated storage hiccup shouldn't make a token refresh
/// that actually succeeded masquerade as a credential failure. The next
/// sync attempt will simply refresh again.
///
/// If the refresh exchange itself fails (checklist item 6: a stale --
/// unused 6+ months -- or revoked refresh token), `connection` is returned
/// unchanged; the ensuing fetch/push then fails with the old/expired token,
/// and `sync::classify_git_error_for` reports it as
/// `SyncFailureCause::CredentialRejected` naming this connection's kind
/// ("sign-in rejected: ...", per `sync::credential_kind_label`) -- no
/// separate cause variant is needed for this case.
///
/// Returns `(connection, save_failed)` -- ticket 13 checklist item 4:
/// `save_failed` is `true` only on the "refreshed but couldn't persist it"
/// path described above, so `lib.rs`'s `perform_sync` can surface
/// `SyncFailureCause::RefreshedSignInNotSaved` as a lower-key warning once
/// the sync attempt that follows actually succeeds with the fresh in-memory
/// pair -- previously this fact was silently dropped on the floor.
pub fn refresh_if_needed(
    repo_root: &std::path::Path,
    keychain: Option<&crate::credential::KeychainBackend>,
    plaintext: &crate::credential::PlaintextStore,
    connection: crate::connection::Connection,
) -> (crate::connection::Connection, bool) {
    use crate::connection_record::{self, CredentialKind, StoreKind};

    if connection.credential_kind() != CredentialKind::OauthSignIn {
        return (connection, false);
    }
    if !needs_refresh(connection.record().token_expiry.as_deref()) {
        return (connection, false);
    }
    let Ok(secret) = OauthSecret::from_bytes(connection.secret_bytes()) else {
        // Corrupt stored secret -- not this function's problem to fix; the
        // ordinary auth path will fail cleanly on it (see
        // `connection::credential_for_kind`'s `OauthSignIn` arm).
        return (connection, false);
    };

    let endpoints = GitHubEndpoints::production();
    let pair = match refresh_token_pair(&endpoints, GITHUB_CLIENT_ID, &secret.refresh_token) {
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

/// The URL to send the user to in order to install the GitHub App on a
/// repository they choose (ticket 06 checklist item 3).
pub fn installation_url() -> String {
    format!("https://github.com/apps/{GITHUB_APP_SLUG}/installations/new")
}

/// What `check_installation` found.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum InstallationStatus {
    Installed,
    NotInstalled { install_url: String },
}

/// One repository as surfaced to the guided connect wizard (ticket 09): just
/// enough to render a pickable list entry (name + private/public indicator)
/// and to hand `clone_url` straight to the existing OAuth-token connect path
/// (`connect_github_oauth`) after a create-new or pick-existing selection --
/// no separate "clone URL" concept, since ticket 09 is connect-only (ticket
/// 10 is clone).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoInfo {
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub clone_url: String,
    pub html_url: String,
}

/// Ticket 09's create-new path: `POST {api_base_url}/user/repos` with `name`
/// and `private` -- private is the caller's responsibility to default to
/// `true` (the wizard never defaults to public). Mirrors
/// `check_installation`'s status-code mapping style: 401/403 is a rejected
/// token, anything else unexpected (422 for a name that's already taken,
/// most commonly) is surfaced with GitHub's own error body so the wizard can
/// show it verbatim rather than a generic failure.
pub fn create_repository(
    endpoints: &GitHubEndpoints,
    access_token: &str,
    name: &str,
    private: bool,
) -> Result<RepoInfo, DeviceFlowError> {
    let url = format!("{}/user/repos", endpoints.api_base_url);
    let body = serde_json::json!({ "name": name, "private": private });
    let (status, body) = post_bearer_json(&url, access_token, &body)?;
    match status {
        200 | 201 => {
            let raw: RawRepoResponse =
                serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
            raw.try_into()
        }
        401 | 403 => Err(DeviceFlowError::Rejected(format!(
            "GitHub rejected the repository creation request with status {status}"
        ))),
        other => Err(DeviceFlowError::UnexpectedResponse(format!(
            "unexpected status {other} creating a repository: {body}"
        ))),
    }
}

/// Ticket 09's pick-existing path: `GET {api_base_url}/user/repos`, sorted by
/// most-recently-updated, first 100 results -- paginating further is
/// explicitly optional per the ticket, and a first reasonable page is enough
/// for a picker list.
pub fn list_repositories(endpoints: &GitHubEndpoints, access_token: &str) -> Result<Vec<RepoInfo>, DeviceFlowError> {
    let url = format!("{}/user/repos?per_page=100&sort=updated", endpoints.api_base_url);
    let (status, body) = get_bearer(&url, access_token)?;
    match status {
        200 => {
            let raw: Vec<RawRepoResponse> =
                serde_json::from_str(&body).map_err(|e| DeviceFlowError::UnexpectedResponse(e.to_string()))?;
            raw.into_iter().map(TryInto::try_into).collect()
        }
        401 | 403 => Err(DeviceFlowError::Rejected(format!(
            "GitHub rejected the repository list request with status {status}"
        ))),
        other => Err(DeviceFlowError::UnexpectedResponse(format!(
            "unexpected status {other} listing repositories: {body}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct RawRepoResponse {
    name: Option<String>,
    full_name: Option<String>,
    private: Option<bool>,
    clone_url: Option<String>,
    html_url: Option<String>,
}

impl TryFrom<RawRepoResponse> for RepoInfo {
    type Error = DeviceFlowError;

    fn try_from(raw: RawRepoResponse) -> Result<Self, Self::Error> {
        let (Some(name), Some(full_name), Some(private), Some(clone_url), Some(html_url)) =
            (raw.name, raw.full_name, raw.private, raw.clone_url, raw.html_url)
        else {
            return Err(DeviceFlowError::UnexpectedResponse(
                "repository response missing required fields".to_string(),
            ));
        };
        Ok(RepoInfo {
            name,
            full_name,
            private,
            clone_url,
            html_url,
        })
    }
}

/// A blocking `POST` with a JSON body and a `Bearer` token, returning
/// (status, body) -- the create-repository counterpart to `get_bearer`.
fn post_bearer_json(url: &str, token: &str, json_body: &serde_json::Value) -> Result<(u16, String), DeviceFlowError> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "cerebrite")
        .json(json_body)
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
    let (Some(access_token), Some(refresh_token), Some(expires_in)) =
        (parsed.access_token, parsed.refresh_token, parsed.expires_in)
    else {
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
        refresh_token,
        access_token_expires_at,
        refresh_token_expires_at,
    })
}

/// RFC 3339 (UTC, second precision) formatter/parser, matching the string
/// shape `connection_record::ConnectionRecord::token_expiry`'s doc comment
/// promises.
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

/// A blocking `POST` with a form-encoded body and `Accept: application/json`
/// (GitHub's device-flow endpoints answer in `application/x-www-form-urlencoded`
/// unless asked otherwise), returning the raw response body. Real network
/// I/O in production; tests point `url` at a local mock server instead of
/// substituting a trait/mock object, matching `connection.rs`'s
/// `basic_auth_fixture` precedent.
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

/// A blocking `GET` with a `Bearer` token, returning (status, body).
fn get_bearer(url: &str, token: &str) -> Result<(u16, String), DeviceFlowError> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "cerebrite")
        .send()
        .map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|e| DeviceFlowError::Network(e.to_string()))?;
    Ok((status, body))
}

/// Splits an HTTPS remote URL's `owner/repo` out of its path, for
/// `check_installation`. `None` for anything that doesn't look like a
/// GitHub `owner/repo(.git)` path.
pub fn owner_repo_from_remote_url(remote_url: &str) -> Option<(String, String)> {
    let without_scheme = remote_url.splitn(2, "://").nth(1).unwrap_or(remote_url);
    let after_host = without_scheme.splitn(2, '/').nth(1)?;
    let trimmed = after_host.trim_end_matches(".git").trim_end_matches('/');
    let mut parts = trimmed.splitn(2, '/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- OauthSecret envelope --

    #[test]
    fn oauth_secret_round_trips_through_bytes() {
        let secret = OauthSecret {
            access_token: "gho_abc".to_string(),
            refresh_token: "ghr_def".to_string(),
        };
        let bytes = secret.to_bytes();
        assert_eq!(OauthSecret::from_bytes(&bytes).unwrap(), secret);
    }

    // -- owner_repo_from_remote_url --

    #[test]
    fn owner_repo_parses_a_normal_https_github_url() {
        assert_eq!(
            owner_repo_from_remote_url("https://github.com/dawid/notes.git"),
            Some(("dawid".to_string(), "notes".to_string()))
        );
    }

    #[test]
    fn owner_repo_parses_without_a_trailing_dot_git() {
        assert_eq!(
            owner_repo_from_remote_url("https://github.com/dawid/notes"),
            Some(("dawid".to_string(), "notes".to_string()))
        );
    }

    #[test]
    fn owner_repo_is_none_for_a_url_missing_a_repo_segment() {
        assert_eq!(owner_repo_from_remote_url("https://github.com/dawid"), None);
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
        assert!(matches!(case("incorrect_client_credentials"), PollOutcome::Error(_)));
    }

    #[test]
    fn interpret_token_response_builds_a_token_pair_on_success() {
        let outcome = interpret_token_response(RawTokenResponse {
            access_token: Some("gho_abc".to_string()),
            refresh_token: Some("ghr_def".to_string()),
            expires_in: Some(28800),
            refresh_token_expires_in: Some(15897600),
            error: None,
        });
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "gho_abc");
                assert_eq!(pair.refresh_token, "ghr_def");
                assert!(pair.refresh_token_expires_at.is_some());
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    // -- mock_server: a hand-rolled HTTP/1.1 responder standing in for
    // github.com/api.github.com, exactly mirroring `connection.rs`'s
    // `basic_auth_fixture` / `sync.rs`'s `spawn_401_server` pattern --
    // never touches real GitHub.
    mod mock_server {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        /// Spawns a server that answers every request with `responses[n]`
        /// in order (one response per request received), for scripting a
        /// sequence of outcomes (e.g. two `authorization_pending` polls
        /// then a success). Each entry is (status_line_reason, json body).
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
                        404 => "Not Found",
                        401 => "Unauthorized",
                        403 => "Forbidden",
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

    fn endpoints_for(port: u16) -> GitHubEndpoints {
        let base = format!("http://127.0.0.1:{port}");
        GitHubEndpoints {
            device_code_url: base.clone(),
            token_url: base.clone(),
            api_base_url: base,
        }
    }

    #[test]
    fn request_device_code_parses_a_successful_mock_response() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"device_code":"dc123","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#.to_string(),
        )]);
        let info = request_device_code(&endpoints_for(port), "test-client-id").unwrap();
        assert_eq!(info.device_code, "dc123");
        assert_eq!(info.user_code, "WDJB-MJHT");
        assert_eq!(info.expires_in_secs, 900);
        assert_eq!(info.interval_secs, 5);
    }

    #[test]
    fn request_device_code_surfaces_a_rejected_client_id() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"error":"incorrect_client_credentials","error_description":"placeholder client id"}"#.to_string(),
        )]);
        let result = request_device_code(&endpoints_for(port), "TODO_REGISTER_GITHUB_APP");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    #[test]
    fn poll_once_reports_authorization_pending() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"authorization_pending"}"#.to_string())]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        assert_eq!(outcome, PollOutcome::Pending);
    }

    #[test]
    fn poll_once_reports_slow_down() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"slow_down"}"#.to_string())]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        assert_eq!(outcome, PollOutcome::SlowDown);
    }

    #[test]
    fn poll_once_reports_expired_token() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"expired_token"}"#.to_string())]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        assert_eq!(outcome, PollOutcome::Expired);
    }

    #[test]
    fn poll_once_reports_access_denied() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"access_denied"}"#.to_string())]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        assert_eq!(outcome, PollOutcome::Denied);
    }

    #[test]
    fn poll_once_reports_success_with_a_token_pair() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"access_token":"gho_abc","token_type":"bearer","refresh_token":"ghr_def","expires_in":28800,"refresh_token_expires_in":15897600}"#.to_string(),
        )]);
        let outcome = poll_once(&endpoints_for(port), "client", "dc123");
        match outcome {
            PollOutcome::Success(pair) => {
                assert_eq!(pair.access_token, "gho_abc");
                assert_eq!(pair.refresh_token, "ghr_def");
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
                r#"{"access_token":"gho_abc","refresh_token":"ghr_def","expires_in":28800,"refresh_token_expires_in":15897600}"#
                    .to_string(),
            ),
        ]);
        let device = DeviceCodeInfo {
            device_code: "dc123".to_string(),
            user_code: "WDJB-MJHT".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            // Long enough not to hit the deadline while this test's three
            // short sleeps (interval starts at 1s) run.
            expires_in_secs: 60,
            interval_secs: 1,
        };
        let pair = poll_until_complete(&endpoints_for(port), "client", &device).unwrap();
        assert_eq!(pair.access_token, "gho_abc");
    }

    #[test]
    fn poll_until_complete_surfaces_access_denied() {
        let port = mock_server::spawn(vec![(200, r#"{"error":"access_denied"}"#.to_string())]);
        let device = DeviceCodeInfo {
            device_code: "dc123".to_string(),
            user_code: "WDJB-MJHT".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
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
            user_code: "WDJB-MJHT".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            // Shorter than the interval, so the very first deadline check
            // (after the first sleep) already trips ExpiredToken.
            expires_in_secs: 1,
            interval_secs: 2,
        };
        let result = poll_until_complete(&endpoints_for(port), "client", &device);
        assert_eq!(result, Err(DeviceFlowError::ExpiredToken));
    }

    #[test]
    fn refresh_token_pair_returns_a_rotated_pair() {
        let port = mock_server::spawn(vec![(
            200,
            r#"{"access_token":"gho_new","refresh_token":"ghr_new","expires_in":28800,"refresh_token_expires_in":15897600}"#
                .to_string(),
        )]);
        let pair = refresh_token_pair(&endpoints_for(port), "client", "ghr_old").unwrap();
        assert_eq!(pair.access_token, "gho_new");
        assert_eq!(pair.refresh_token, "ghr_new");
    }

    #[test]
    fn refresh_token_pair_surfaces_a_revoked_refresh_token() {
        // A refresh token that's stale (unused 6+ months) or whose install
        // was revoked comes back as a rejected-grant error, same shape as
        // any other rejected refresh -- ticket 06 checklist item 6.
        let port = mock_server::spawn(vec![(
            200,
            r#"{"error":"bad_refresh_token","error_description":"The refresh token passed is incorrect or expired."}"#.to_string(),
        )]);
        let result = refresh_token_pair(&endpoints_for(port), "client", "ghr_stale");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    #[test]
    fn check_installation_reports_installed_on_200() {
        let port = mock_server::spawn(vec![(200, r#"{"id":1}"#.to_string())]);
        let status = check_installation(&endpoints_for(port), "gho_abc", "dawid", "notes").unwrap();
        assert_eq!(status, InstallationStatus::Installed);
    }

    #[test]
    fn check_installation_reports_not_installed_with_an_install_url_on_404() {
        let port = mock_server::spawn(vec![(404, r#"{"message":"Not Found"}"#.to_string())]);
        let status = check_installation(&endpoints_for(port), "gho_abc", "dawid", "notes").unwrap();
        match status {
            InstallationStatus::NotInstalled { install_url } => {
                assert!(install_url.contains("/installations/new"));
                assert!(install_url.contains(GITHUB_APP_SLUG));
            }
            other => panic!("expected NotInstalled, got {other:?}"),
        }
    }

    #[test]
    fn check_installation_surfaces_a_rejected_token() {
        let port = mock_server::spawn(vec![(401, r#"{"message":"Bad credentials"}"#.to_string())]);
        let result = check_installation(&endpoints_for(port), "bad-token", "dawid", "notes");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    // -- create_repository / list_repositories (ticket 09) --

    #[test]
    fn create_repository_parses_a_successful_response() {
        let port = mock_server::spawn(vec![(
            201,
            r#"{"name":"notes","full_name":"dawid/notes","private":true,"clone_url":"https://github.com/dawid/notes.git","html_url":"https://github.com/dawid/notes"}"#.to_string(),
        )]);
        let repo = create_repository(&endpoints_for(port), "gho_abc", "notes", true).unwrap();
        assert_eq!(repo.name, "notes");
        assert_eq!(repo.full_name, "dawid/notes");
        assert!(repo.private);
        assert_eq!(repo.clone_url, "https://github.com/dawid/notes.git");
    }

    #[test]
    fn create_repository_surfaces_a_name_already_taken_rejection() {
        // GitHub answers a name collision with 422, not 401/403 -- this must
        // surface as an error the wizard can show, not a parse panic.
        let port = mock_server::spawn(vec![(
            422,
            r#"{"message":"Repository creation failed.","errors":[{"message":"name already exists on this account"}]}"#
                .to_string(),
        )]);
        let result = create_repository(&endpoints_for(port), "gho_abc", "notes", true);
        assert!(matches!(result, Err(DeviceFlowError::UnexpectedResponse(_))));
    }

    #[test]
    fn create_repository_surfaces_a_rejected_token() {
        let port = mock_server::spawn(vec![(401, r#"{"message":"Bad credentials"}"#.to_string())]);
        let result = create_repository(&endpoints_for(port), "bad-token", "notes", true);
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }

    #[test]
    fn list_repositories_parses_a_successful_response() {
        let port = mock_server::spawn(vec![(
            200,
            r#"[{"name":"notes","full_name":"dawid/notes","private":true,"clone_url":"https://github.com/dawid/notes.git","html_url":"https://github.com/dawid/notes"},
                {"name":"public-thing","full_name":"dawid/public-thing","private":false,"clone_url":"https://github.com/dawid/public-thing.git","html_url":"https://github.com/dawid/public-thing"}]"#.to_string(),
        )]);
        let repos = list_repositories(&endpoints_for(port), "gho_abc").unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "notes");
        assert!(repos[0].private);
        assert!(!repos[1].private);
    }

    #[test]
    fn list_repositories_surfaces_a_rejected_token() {
        let port = mock_server::spawn(vec![(401, r#"{"message":"Bad credentials"}"#.to_string())]);
        let result = list_repositories(&endpoints_for(port), "bad-token");
        assert!(matches!(result, Err(DeviceFlowError::Rejected(_))));
    }
}
