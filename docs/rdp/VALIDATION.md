# Validation — upstream 0.8.0 integration (2026-10-21)

Upstream master: `089e003` (0.8.0, plus website changes after the release tag). Existing remote 0.7.1 synchronization was fast-forwarded before merging; no remote work was overwritten.

The merge retained RDP and the new SFTP/Yandex Cloud modules in registration and test fixtures. The previous 0.7.1 build failed because an SSH dialog test selected the hidden RDP add button; its selector is now scoped to the active section. No application behavior was bypassed.

Validated locally on Arch Linux x86_64 with Rust 1.93.1:

- Production TypeScript/Vite build passed.
- 98 unit tests passed.
- 130 Rust tests passed; 4 external-server Passbolt tests remain ignored as in upstream.
- Clippy (`--lib --tests -- -D warnings`) passed.
- All 205 Playwright browser tests passed, including RDP and the SSH Enter regression.
- Translation check passed: 5,413 texts, 2,020 dictionary entries.
- Author-link check passed (216 files).
- RDP uses the shared upstream atomic JSON persistence and backup recovery.
- Existing signing key, fork updater endpoint, RDP icon and password pipe transport were retained.

No Windows server or credentials were supplied. Live RDP authentication and server-specific behavior remain unverified.
