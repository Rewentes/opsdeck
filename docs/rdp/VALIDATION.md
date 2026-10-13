# Validation — upstream 0.7.0 integration (2026-10-13)

Upstream master: `ac4fcb7` (0.7.0). The merge preserved the RDP module, monitor-arrow icon, personal update endpoint and existing signing public key. The build.rs conflict was resolved by retaining the package-format rebuild trigger and the upstream Windows manifest fix. RDP KeePass reads now use `Use::User`, matching the new upstream activity tracking.

Validated locally on Arch Linux x86_64 with upstream Rust 1.93.1:

- TypeScript and production Vite build passed.
- 88 unit tests passed.
- 92 Rust tests passed; 4 Passbolt live integration tests require an external server and were ignored by upstream.
- All 140 Playwright tests passed, including RDP and new upstream screens.
- The web-panel overlay test now explicitly controls when the first native show finishes, replacing a timing assumption that failed under parallel load. Its assertions remain intact.
- Translation check: 4,822 texts checked, 1,809 dictionary entries.
- Author-link check passed (196 files).
- RDP registration, selected icon and personal signing key/endpoint checked after merge.
- GitHub upstream synchronization passed after the manual conflict resolution; fork master remains an exact upstream copy.

No remote RDP server or credentials were supplied. Live Windows authentication and server-specific behavior remain unverified.
