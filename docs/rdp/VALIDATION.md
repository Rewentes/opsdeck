# Validation — OpsDeck 0.8.2, NetBird and RDP (2026-10-26)

Upstream master: `c17df05` (0.8.2). Merge conflicts in module registration and UI fixtures retained both RDP/NetBird and upstream AI-agent/popout behavior.

Local validation on Arch x86_64, Rust 1.93.1:

- Production TypeScript/Vite build passed.
- 125 unit tests passed.
- 163 Rust tests passed; 4 external-server Passbolt tests remain ignored upstream.
- Clippy (`--lib --tests -- -D warnings`) passed.
- 242 Playwright tests passed, including 13 focused NetBird/RDP tests.
- Translation check passed (5,798 texts / 2,194 dictionary entries); author-link check passed. NetBird UI and errors have English translations.
- The existing upstream interrupted-download test fixture was synchronized: it holds the first four partial responses open until all four requests arrive. This prevents early cancellation from misclassifying resumed requests; download implementation and assertions are unchanged.
- NetBird tests cover duplicate profile names with stable IDs, URL/argument validation, missing/unauthenticated profiles, readiness checks, CLI failure output sanitization, connection reuse, UI confirmations, disabled duplicate operations, optional/removed RDP bindings and failure reporting. A temporary fake CLI exercises the real process adapter without altering the developer's VPN.
- Native NetBird 0.79.0 help/profile/status commands were inspected read-only; no live VPN switch, profile mutation or browser authentication was performed during development.
- Existing signing key, fork updater endpoint, RDP icon, stdin-only password transport and atomic settings persistence are retained.

No Windows RDP server or test VPN account was supplied. Live SSO and Windows authentication remain unverified.
