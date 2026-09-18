# Where credentials live at rest

Type: grilling
Status: open
Blocked by: 04, 06

## Question

Given the mechanisms that ticket 06 settled on and the storage options ticket 04 surveyed, where do Cerebrite's credentials actually live — and what is the honest security claim?

Hard to reverse (migrating stored secrets after release is genuinely painful), surprising without context, and a real trade-off between portability and protection. This should produce an ADR.

Settle:

- **The chosen mechanism**, per platform, including what happens on Windows, on a Linux box with no secret service, and what the Android path will be when it is eventually built.
- **The fallback when it is unavailable.** A keychain that fails on a user's machine must not leave sync permanently broken with no explanation. Refuse and explain, degrade to a less protected store with visible consent, or something else — but decided, not accidental.
- **The security claim.** State plainly what an attacker with read access to the user's home directory gets, and make sure the UI never promises more than that. If the real answer is "a file-permission-protected token", say so rather than implying encryption that does not exist.
- **Scope of what is stored.** Just the credential, or also the remote URL, the chosen mechanism, and the provider identity? Some of that is not secret and may belong in `settings.json` next to `vaultPath` and `theme`; splitting it deliberately is better than putting everything in the most awkward store.
- **Revocation and clearing.** How a user disconnects a vault from its provider and is genuinely confident the credential is gone.
