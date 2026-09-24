// "Commit as" author identity (ticket 08), implementing the resolved
// decision recorded in
// `.scratch/git-provider-integration/issues/11-git-author-identity-product.md`
// (grounded in the facts from
// `.scratch/git-provider-integration/issues/10-git-author-identity-facts.md`):
//
// **Cerebrite never commits under an author the user has not seen and
// confirmed.** The `Cerebrite <cerebrite@local>` fallback that used to live
// in `vault.rs::commit_all` and `sync.rs`'s merge-commit path is gone
// outright (not reordered, not kept as a last resort) -- committing without
// a confirmed author is now a hard error there, surfaced via
// `Repository::signature`'s own `NotFound` failure rather than anything
// invented here.
//
// The author belongs to the **vault**, not the connection (a vault with no
// connection still commits, ADR-0006), and lives exclusively in **repo-local
// git config** -- `user.name`/`user.email` in the repository's `.git/config`
// -- never in `.git/cerebrite/connection.json` (see `connection_record.rs`'s
// module doc comment, which was amended by ticket 11 to say so explicitly),
// never in the global `~/.gitconfig`, and never synced: a `.git/config` file
// never travels with the repository's tracked content, so a second device
// cloning the same vault always starts with no repo-local author of its own
// and goes through its own confirmation, with values only *suggested*
// (checklist item 9). Author and committer are always the same value --
// nothing in this codebase ever reads a separate committer identity, so
// writing `user.name`/`user.email` once (`confirm`, below) covers both:
// `Repository::signature()` (what `vault::commit_all` and `sync::run_sync`'s
// merge commit both call) builds one `Signature` used for both roles.
#![allow(dead_code)]

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A confirmed or suggested commit author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

/// Where a `PrefillResult`'s author came from -- surfaced to the frontend so
/// the "Commit as" step can say *why* a value showed up (ticket 11: "always
/// shown and always editable", not silently applied) rather than just
/// displaying it unexplained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefillSource {
    RepoLocal,
    Global,
    Provider,
    /// The base case for access-token/SSH connections (and OAuth sign-in
    /// before it's happened) with nothing to draw on at all -- ticket 11's
    /// "empty" tier, never a placeholder identity.
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefillResult {
    pub author: Option<CommitAuthor>,
    pub source: PrefillSource,
}

/// Reads `user.name`/`user.email` from `repo_root`'s repo-local git config
/// only (`ConfigLevel::Local`) -- deliberately never falls through to the
/// global config the way a plain `repo.config()` lookup would, since the
/// prefill chain needs to tell the two tiers apart (ticket 11's precedence:
/// repo-local *wins over* global, it doesn't merge with it). `None` if
/// either half is unset or blank.
pub fn read_repo_local(repo_root: &Path) -> anyhow::Result<Option<CommitAuthor>> {
    let repo = git2::Repository::open(repo_root)?;
    let local = repo.config()?.open_level(git2::ConfigLevel::Local)?;
    read_from_config(&local)
}

/// Reads `user.name`/`user.email` from the machine's global/system git
/// config (`~/.gitconfig`, `$XDG_CONFIG_HOME/git/config`, etc, via git2's
/// normal default layering) -- the second rung of the prefill chain. Never
/// written to; only `confirm` (repo-local only) ever writes anything.
///
/// Not exercised directly by this module's own tests: which values (if any)
/// this returns depends on the *real* machine's git installation, which
/// this crate's test suite must not depend on -- see `prefill`'s tests,
/// which cover the precedence chain as a pure function over
/// already-resolved `Option<CommitAuthor>` inputs instead, and
/// `vault.rs`/`sync.rs`'s `isolate_from_host_git_config` test helper, which
/// exists specifically because the sandbox this ticket was implemented in
/// *does* have a real global `user.name`/`user.email` set.
pub fn read_global() -> anyhow::Result<Option<CommitAuthor>> {
    let config = git2::Config::open_default()?;
    read_from_config(&config)
}

fn read_from_config(config: &git2::Config) -> anyhow::Result<Option<CommitAuthor>> {
    let name = config.get_string("user.name").ok();
    let email = config.get_string("user.email").ok();
    match (name, email) {
        (Some(name), Some(email)) if !name.trim().is_empty() && !email.trim().is_empty() => {
            Ok(Some(CommitAuthor { name, email }))
        }
        _ => Ok(None),
    }
}

/// The prefill precedence chain (ticket 11's "What is suggested" section,
/// checklist item 3): repo-local -> global -> provider (OAuth sign-in only)
/// -> empty. A pure function over already-resolved inputs -- callers
/// (`lib.rs`'s Tauri commands) do the actual repo-local/global/provider
/// lookups and hand the results in here, which is what makes the precedence
/// logic itself trivially unit-testable without touching git2, the
/// filesystem, or the network.
pub fn prefill(
    repo_local: Option<CommitAuthor>,
    global: Option<CommitAuthor>,
    provider_identity: Option<CommitAuthor>,
) -> PrefillResult {
    if let Some(author) = repo_local {
        return PrefillResult {
            author: Some(author),
            source: PrefillSource::RepoLocal,
        };
    }
    if let Some(author) = global {
        return PrefillResult {
            author: Some(author),
            source: PrefillSource::Global,
        };
    }
    if let Some(author) = provider_identity {
        return PrefillResult {
            author: Some(author),
            source: PrefillSource::Provider,
        };
    }
    PrefillResult {
        author: None,
        source: PrefillSource::Empty,
    }
}

/// Writes `author` to `repo_root`'s repo-local git config only
/// (`user.name`/`user.email` in `.git/config`) -- ticket 11 checklist item
/// 4: "Confirming writes the value repo-locally. A later change to the
/// global config therefore never silently changes a vault's author." That
/// guarantee falls out for free from *where* this writes: `open_level(Local)`
/// only ever touches `.git/config`, so nothing here can reach `~/.gitconfig`,
/// and `Repository::signature()`'s own layered lookup already prefers a
/// repo-local value over a global one -- there's no separate "don't let
/// global override" mechanism to build.
pub fn confirm(repo_root: &Path, author: &CommitAuthor) -> anyhow::Result<()> {
    let repo = git2::Repository::open(repo_root)?;
    let mut local = repo.config()?.open_level(git2::ConfigLevel::Local)?;
    local.set_str("user.name", &author.name)?;
    local.set_str("user.email", &author.email)?;
    Ok(())
}

/// What `confirm_commit_author` (the Tauri command) hands back once a name
/// and email pass validation -- `warning` is ticket 11's "warn, never
/// block" case for a domain that can't be real.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationOutcome {
    pub warning: Option<String>,
}

/// A hard validation failure -- ticket 11's only two: an empty name, or an
/// email missing `@` or carrying a domain with `<`, `>`, or a line break.
/// Never fired for an unrealistic-but-well-formed domain (`.local`,
/// `localhost`, no dot) -- that's `ValidationOutcome::warning` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    EmptyName,
    InvalidEmail(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::EmptyName => write!(f, "a name is required"),
            ValidationError::InvalidEmail(detail) => write!(f, "not a valid email address: {detail}"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// Ticket 11's "Validation" section, checklist item 7: a non-empty name, and
/// an email with an `@` and a domain containing no `<`, `>`, or line breaks
/// -- the mailbox itself is never verified, and an unrealistic-but-otherwise-
/// valid domain (`.local`, `localhost`, no dot at all -- the permanent-loss
/// case ticket 10 found) only ever produces a warning, never a hard error.
pub fn validate(name: &str, email: &str) -> Result<ValidationOutcome, ValidationError> {
    if name.trim().is_empty() {
        return Err(ValidationError::EmptyName);
    }

    let Some(at_pos) = email.find('@') else {
        return Err(ValidationError::InvalidEmail("missing '@'".to_string()));
    };
    let local_part = &email[..at_pos];
    let domain = &email[at_pos + 1..];
    if local_part.trim().is_empty() {
        return Err(ValidationError::InvalidEmail("missing the part before '@'".to_string()));
    }
    if domain.is_empty() {
        return Err(ValidationError::InvalidEmail("missing a domain".to_string()));
    }
    if domain.contains('<') || domain.contains('>') || domain.contains('\n') || domain.contains('\r') {
        return Err(ValidationError::InvalidEmail(
            "the domain can't contain '<', '>', or a line break".to_string(),
        ));
    }

    let warning = if looks_unreal(domain) {
        Some("commits with this address can't be linked to any account".to_string())
    } else {
        None
    };
    Ok(ValidationOutcome { warning })
}

/// Ticket 11's "a domain that cannot be real" list: `.local`, `localhost`,
/// or no dot at all (which also covers bare `localhost`, so it isn't its own
/// special case).
fn looks_unreal(domain: &str) -> bool {
    let lower = domain.trim().to_lowercase();
    lower.ends_with(".local") || !lower.contains('.')
}

/// Builds GitHub's private/noreply commit email (ticket 10/11): the only
/// email variant that (a) needs no extra consent scope, (b) counts as a
/// contribution, (c) survives username changes, and (d) can never trip
/// GitHub's "block command line pushes that expose my email" push guard --
/// see `github_oauth.rs`'s sibling doc comments for the same fact from the
/// OAuth module's side. Valid for any account created after 2017-07-18; the
/// map decided this is always the right choice for a *suggested* address
/// (ticket 11: "the real address is never suggested").
pub fn github_noreply_email(id: u64, login: &str) -> String {
    format!("{id}+{login}@users.noreply.github.com")
}

/// The minimal shape read out of GitHub's `GET /user` (ticket 11: "needs no
/// permission at all" on a GitHub App user-to-server token) -- only what's
/// needed to build the noreply email and a display name.
#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    id: u64,
    login: String,
    name: Option<String>,
}

/// Ticket 11 checklist item 3's GitHub half: `GET {api_base_url}/user` with
/// the OAuth sign-in access token, no extra scope, and the real email is
/// never even read out of the response -- only `id`/`login` (for the
/// noreply address) and `name` (falling back to `login` when the profile has
/// no display name set) ever leave this function.
pub fn fetch_github_identity(api_base_url: &str, access_token: &str) -> anyhow::Result<CommitAuthor> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{api_base_url}/user"))
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "cerebrite")
        .send()?;
    let user: GitHubUserResponse = response.json()?;
    let name = user
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| user.login.clone());
    Ok(CommitAuthor {
        name,
        email: github_noreply_email(user.id, &user.login),
    })
}

/// The minimal shape read out of GitLab's `GET /user` -- `commit_email` is
/// literally the address GitLab wants used as a commit author (ticket 10);
/// `None` (or blank) means this account has nothing suggestable, and the
/// caller falls through the rest of the prefill chain.
#[derive(Debug, Deserialize)]
struct GitLabUserResponse {
    name: String,
    commit_email: Option<String>,
}

/// Ticket 11 checklist item 3's GitLab half: `GET {api_base_url}/user` under
/// whichever scope the connection already holds (`api`, or `read_user` per
/// the still-pending GitLab contingency documented in `gitlab_oauth.rs`) --
/// no extra scope is requested by this function itself, it just reads
/// whatever the existing token can see.
pub fn fetch_gitlab_identity(api_base_url: &str, access_token: &str) -> anyhow::Result<Option<CommitAuthor>> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{api_base_url}/user"))
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "cerebrite")
        .send()?;
    let user: GitLabUserResponse = response.json()?;
    let email = user.commit_email.filter(|e| !e.trim().is_empty());
    Ok(email.map(|email| CommitAuthor { name: user.name, email }))
}

/// Test-only fixture helper: confirms a fixed, throwaway repo-local author
/// for `repo_root` -- used across this crate's other test modules
/// (`vault.rs`, `sync.rs`, `trash.rs`, `redirects.rs`, `connection.rs`,
/// `lib.rs`) so their fixtures don't depend on this machine's real global
/// git config (which the sandbox this ticket was implemented in happens to
/// have set) to make `commit_all`/`run_sync`'s merge commit succeed. Kept
/// `pub(crate)` (not `#[cfg(test)] mod tests`-private) specifically so other
/// modules' test code can call it.
#[cfg(test)]
pub(crate) fn confirm_test_author(repo_root: &Path) {
    confirm(
        repo_root,
        &CommitAuthor {
            name: "Test User".to_string(),
            email: "test-user@example.com".to_string(),
        },
    )
    .expect("confirming a test fixture's commit author");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn author(name: &str, email: &str) -> CommitAuthor {
        CommitAuthor {
            name: name.to_string(),
            email: email.to_string(),
        }
    }

    // -- prefill: the precedence chain as a pure function --

    #[test]
    fn prefill_prefers_repo_local_over_everything_else() {
        let result = prefill(
            Some(author("Repo Local", "local@example.com")),
            Some(author("Global", "global@example.com")),
            Some(author("Provider", "provider@example.com")),
        );
        assert_eq!(result.author, Some(author("Repo Local", "local@example.com")));
        assert_eq!(result.source, PrefillSource::RepoLocal);
    }

    #[test]
    fn prefill_falls_back_to_global_when_repo_local_is_absent() {
        let result = prefill(
            None,
            Some(author("Global", "global@example.com")),
            Some(author("Provider", "provider@example.com")),
        );
        assert_eq!(result.author, Some(author("Global", "global@example.com")));
        assert_eq!(result.source, PrefillSource::Global);
    }

    #[test]
    fn prefill_falls_back_to_the_provider_identity_when_repo_local_and_global_are_both_absent() {
        let result = prefill(None, None, Some(author("Provider", "provider@example.com")));
        assert_eq!(result.author, Some(author("Provider", "provider@example.com")));
        assert_eq!(result.source, PrefillSource::Provider);
    }

    #[test]
    fn prefill_is_empty_when_nothing_is_available_access_token_or_ssh_connections() {
        let result = prefill(None, None, None);
        assert_eq!(result.author, None);
        assert_eq!(result.source, PrefillSource::Empty);
    }

    // -- read_repo_local / confirm: real git2 round trip --

    #[test]
    fn read_repo_local_is_none_on_a_fresh_repo_with_no_config_set() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        assert_eq!(read_repo_local(dir.path()).unwrap(), None);
    }

    #[test]
    fn confirm_then_read_repo_local_round_trips() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        let the_author = author("Dawid Rusin", "d.rusin@example.com");

        confirm(dir.path(), &the_author).unwrap();

        assert_eq!(read_repo_local(dir.path()).unwrap(), Some(the_author));
    }

    #[test]
    fn confirm_writes_only_dot_git_config_not_any_tracked_or_connection_file() {
        // Ticket 08 checklist item 9 / the ticket's own item 9 in the
        // implementation issue: nothing about the author may end up
        // syncable (a tracked `.cerebrite/` file) or in
        // `.git/cerebrite/connection.json` (ticket 11 amended that file's
        // shape to drop the author field entirely -- see
        // `connection_record.rs`). Confirming must only ever touch
        // `.git/config`.
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();

        confirm(dir.path(), &author("Dawid Rusin", "d.rusin@example.com")).unwrap();

        let config_contents = std::fs::read_to_string(dir.path().join(".git/config")).unwrap();
        assert!(config_contents.contains("d.rusin@example.com"));
        assert!(!dir.path().join(".cerebrite").exists());
        assert!(!dir.path().join(".git/cerebrite/connection.json").exists());
    }

    #[test]
    fn confirm_overwrites_a_previously_confirmed_author() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        confirm(dir.path(), &author("Old Name", "old@example.com")).unwrap();

        confirm(dir.path(), &author("New Name", "new@example.com")).unwrap();

        assert_eq!(
            read_repo_local(dir.path()).unwrap(),
            Some(author("New Name", "new@example.com"))
        );
    }

    // -- validate --

    #[test]
    fn validate_accepts_an_ordinary_name_and_email() {
        let outcome = validate("Dawid Rusin", "d.rusin@example.com").unwrap();
        assert_eq!(outcome.warning, None);
    }

    #[test]
    fn validate_rejects_an_empty_name() {
        assert_eq!(validate("   ", "d.rusin@example.com"), Err(ValidationError::EmptyName));
    }

    #[test]
    fn validate_rejects_an_email_with_no_at_sign() {
        assert!(matches!(
            validate("Dawid Rusin", "not-an-email"),
            Err(ValidationError::InvalidEmail(_))
        ));
    }

    #[test]
    fn validate_rejects_an_email_whose_domain_contains_angle_brackets() {
        assert!(matches!(
            validate("Dawid Rusin", "d.rusin@example.com<script>"),
            Err(ValidationError::InvalidEmail(_))
        ));
    }

    #[test]
    fn validate_rejects_an_email_whose_domain_contains_a_line_break() {
        assert!(matches!(
            validate("Dawid Rusin", "d.rusin@example.com\nInjected-Header: x"),
            Err(ValidationError::InvalidEmail(_))
        ));
    }

    #[test]
    fn validate_warns_without_blocking_on_a_dot_local_domain() {
        let outcome = validate("Dawid Rusin", "dawid@cerebrite.local").unwrap();
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn validate_warns_without_blocking_on_bare_localhost() {
        let outcome = validate("Dawid Rusin", "dawid@localhost").unwrap();
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn validate_warns_without_blocking_on_a_domain_with_no_dot_at_all() {
        let outcome = validate("Dawid Rusin", "dawid@myhost").unwrap();
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn validate_does_not_warn_on_an_ordinary_multi_label_domain() {
        let outcome = validate("Dawid Rusin", "dawid@users.noreply.github.com").unwrap();
        assert_eq!(outcome.warning, None);
    }

    // -- github_noreply_email --

    #[test]
    fn github_noreply_email_builds_the_documented_shape() {
        assert_eq!(
            github_noreply_email(12345, "dawid"),
            "12345+dawid@users.noreply.github.com"
        );
    }

    // -- fetch_github_identity / fetch_gitlab_identity: against a local mock
    // server, mirroring `github_oauth.rs`/`gitlab_oauth.rs`'s own
    // `mock_server` pattern -- never touches a real provider.

    mod mock_server {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        pub fn spawn(body: String) -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            std::thread::spawn(move || {
                for stream in listener.incoming().take(5) {
                    let Ok(mut stream) = stream else { continue };
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            });
            port
        }
    }

    #[test]
    fn fetch_github_identity_builds_the_noreply_email_from_get_user() {
        let port = mock_server::spawn(r#"{"id":12345,"login":"dawid","name":"Dawid Rusin"}"#.to_string());
        let identity = fetch_github_identity(&format!("http://127.0.0.1:{port}"), "gho_abc").unwrap();
        assert_eq!(identity.name, "Dawid Rusin");
        assert_eq!(identity.email, "12345+dawid@users.noreply.github.com");
    }

    #[test]
    fn fetch_github_identity_falls_back_to_login_when_no_display_name_is_set() {
        let port = mock_server::spawn(r#"{"id":12345,"login":"dawid","name":null}"#.to_string());
        let identity = fetch_github_identity(&format!("http://127.0.0.1:{port}"), "gho_abc").unwrap();
        assert_eq!(identity.name, "dawid");
    }

    #[test]
    fn fetch_gitlab_identity_reads_commit_email_from_get_user() {
        let port = mock_server::spawn(
            r#"{"name":"Dawid Rusin","commit_email":"d.rusin@example.com"}"#.to_string(),
        );
        let identity = fetch_gitlab_identity(&format!("http://127.0.0.1:{port}"), "glpat_abc")
            .unwrap()
            .unwrap();
        assert_eq!(identity.name, "Dawid Rusin");
        assert_eq!(identity.email, "d.rusin@example.com");
    }

    #[test]
    fn fetch_gitlab_identity_is_none_when_commit_email_is_absent() {
        let port = mock_server::spawn(r#"{"name":"Dawid Rusin","commit_email":null}"#.to_string());
        let identity = fetch_gitlab_identity(&format!("http://127.0.0.1:{port}"), "glpat_abc").unwrap();
        assert_eq!(identity, None);
    }
}
