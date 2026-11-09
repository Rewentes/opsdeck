# Validation — OpsDeck 0.8.4, NetBird browser sign-in and RDP (2026-11-09)

Upstream master: `76fc4f0` (0.8.4). The merge preserves fork signing, RDP, NetBird and SSH notes. The upstream environment-copy feature now also copies RDP profiles and NetBird URL overrides, without keyring passwords or NetBird identities.

Local validation on Arch x86_64, Rust 1.93.1:

- Production TypeScript/Vite build passed.
- 133 unit tests passed.
- 212 Rust tests passed; 4 live Passbolt tests remain ignored upstream. Clippy with warnings denied passed.
- 285 Playwright tests passed. After the final sign-in event scoping change, 20 focused NetBird, RDP and environment tests passed.
- Translation check (6,003 texts, 2,306 dictionary entries) and author-link check (272 files) passed.
- The fake NetBird CLI confirms that the sign-in URL is visible before the CLI exits and disappears on failure; the parser only accepts the announced HTTPS/loopback link and optional device code, without forwarding arbitrary CLI output.
- The browser test verifies that the pending sign-in panel offers the link, code and system-browser button, and clears when sign-in ends. Existing RDP readiness gating remains covered.
- No live NetBird login, VPN switch or Windows RDP authentication was performed.
