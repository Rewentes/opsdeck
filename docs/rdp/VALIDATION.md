# Validation

Validated locally on the user's Arch Linux x86_64 environment, FreeRDP 3.32.1.

- `npm run build`: TypeScript and production Vite bundle passed.
- `npm test`: 34 unit tests passed.
- `cargo test --lib`: 71 tests passed, including RDP option generation, IPv6, credential newline/NUL rejection, and RDP version ordering.
- `cargo clippy --lib --tests -- -D warnings`: passed.
- Browser suite: initial 102/105 passed; fresh-module expectations updated for RDP, and two tests interrupted by development-server reload passed when repeated. New RDP and English checks passed after translation additions.
- `npm run i18n:check`, author-link checks, workflow YAML parsing and shell syntax checks passed.
- Installed `xfreerdp3 /args-from:stdin` accepted the newline-delimited argument protocol.
- Temporary Git repositories: successful upstream merge preserved RDP changes, advanced master to upstream; conflicting upstream addition aborted, left no in-progress merge, and changed neither remote branch.

No remote RDP server or login was supplied. Live Windows authentication, certificate negotiation, clipboard/audio redirection and monitor behavior must be checked on the user's server.
