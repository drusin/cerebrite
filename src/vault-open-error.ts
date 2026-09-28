/**
 * Ticket 09 checklist item 8: translates `ensure_git_repo`'s nested-repo
 * refusal (`vault.rs`: "'<picked>' is inside an existing git repository
 * rooted at '<root>'. Pick that folder instead of a folder nested inside
 * it.") into copy a non-technical user can act on, rather than showing the
 * raw Rust error string verbatim. This is the only "adoption"/"refusal"
 * case `ensure_git_repo` actually surfaces as an `Err` -- adopting an
 * existing content-bearing repo (README, source files) is a silent
 * success, not an error, so there is nothing to translate for that case.
 *
 * Applied everywhere a vault-open error reaches the user: the vault picker
 * (`surfaces/vault-picker/`), "Change folder…" in still-vanilla Settings,
 * and the remembered-vault auto-open at launch -- since that's the only
 * place this particular error can occur, not inside the connect wizard
 * itself, which only ever operates on an already-open vault. Every other
 * rejection (including the Android "no folder picker" one -- see
 * `pick_vault_folder` in lib.rs) is shown verbatim.
 */
export function friendlyVaultOpenError(raw: string): string {
  const match = raw.match(/is inside an existing git repository rooted at '([^']+)'/);
  if (!match) return raw;
  const root = match[1];
  return `That folder is inside an existing repository. Pick the repository's own top-level folder instead: "${root}".`;
}
