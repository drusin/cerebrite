# 14: Disconnect & credential revocation

**What to build:** Per [ticket 07](../../git-provider-integration/issues/07-credential-storage-decision.md), a way for the user to genuinely disconnect a vault from its provider and be confident the credential is gone, with honest per-kind guidance about revoking it at the source.

From the user's perspective: Settings › Sync has a "Disconnect" action that deletes the stored secret, the connection record, and its index entry. For GitLab, Cerebrite also revokes the refresh token at the provider directly (no secret needed for this). For GitHub, the dialog links to Settings → Applications → Authorized GitHub Apps, since revocation there needs a client secret Cerebrite doesn't hold. For access tokens and SSH keys, the dialog links to the host's token/key management page. The dialog never claims a revocation that didn't actually happen. Separately, on startup, Settings offers to clean up any stored credential whose repository no longer exists on disk, and a "Remove all stored Cerebrite credentials" action is available.

**Blocked by:** 02, 03

- [ ] "Disconnect" in Settings › Sync deletes the keychain/plaintext secret, `.git/cerebrite/connection.json`, and the `settings.json` index entry
- [ ] GitLab: Disconnect also calls `/oauth/revoke` for the refresh token (public-client, no secret needed) and confirms success before telling the user it's revoked
- [ ] GitHub: Disconnect dialog explains revocation needs a client secret Cerebrite doesn't have, and links to Settings → Applications → Authorized GitHub Apps
- [ ] Access token / SSH key: Disconnect dialog links to the host's token-management or SSH-key page (by provider, where known)
- [ ] The dialog's copy never states a revocation happened unless it actually did
- [ ] At startup, orphaned credential entries (repository no longer present on disk) are detected and offered for cleanup
- [ ] Settings has a "Remove all stored Cerebrite credentials" action, with a confirmation step
- [ ] Test: disconnecting removes all three artifacts (secret, connection record, index entry) and a subsequent sync attempt correctly reports "not connected", not a stale-credential error
