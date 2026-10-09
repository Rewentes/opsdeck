# Validation — upstream 0.6.2 integration (2026-10-09)

Upstream master: `775a880` (0.6.2). The merge preserved the RDP module, monitor-arrow icon, personal update endpoint and existing signing public key. One conflict in `src/styles.css` was resolved by retaining both the RDP styles and upstream notes/Kubernetes styles.

Validated locally on the user's Arch Linux x86_64 environment:

- `npm run build`: TypeScript and production Vite bundle passed.
- `npm test`: 41 unit tests passed.
- `cargo test --lib`: 78 tests passed, including RDP option generation, IPv6, credential newline/NUL rejection and RDP build version ordering.
- `cargo clippy --lib --tests -- -D warnings`: passed.
- Full Playwright suite: 114 browser tests passed, including RDP, English translation and the new upstream Kubernetes/KeePass behavior.
- Translation check: 3,729 texts checked, 1,505 dictionary entries.
- Author-link check, workflow YAML parsing and shell syntax checks passed.
- RDP module registration, selected icon and personal signing key/endpoint were explicitly checked after the merge.

The initial fork setup also validated FreeRDP 3.32.1's stdin argument protocol and safe synchronization using temporary Git repositories: successful merges preserved RDP changes, while conflicts changed neither remote branch.

No remote RDP server or credentials were supplied. Live Windows authentication and server-specific behavior remain unverified.
