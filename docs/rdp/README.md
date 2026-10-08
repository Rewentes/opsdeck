# Personal OpsDeck RDP fork (Arch Linux)

Upstream: https://github.com/LeoAlecksey/opsdeck (master).
Personal version: https://github.com/Rewentes/opsdeck/tree/feature/rdp.
Initial base: `95b3d47266b337e6b354bdaf84f513c79a999742`, upstream 0.6.1, 2026-10-08.

## Install on Arch

Download `opsdeck-rdp-*.pkg.tar.zst` from the personal fork's Releases or the `rdp-arch` Actions artifact, then:

```sh
sudo pacman -U ./opsdeck-rdp-*.pkg.tar.zst
```

The package declares `freerdp`, `gtk3`, `webkit2gtk-4.1`, `libayatana-appindicator` and `xdg-utils` dependencies. A Secret Service provider (for example gnome-keyring) must run in the desktop session to store passwords. KeePass must be unlocked in OpsDeck before using bound entries. On Wayland install/enable XWayland; FreeRDP opens an independent X11 window.

The package conflicts with other OpsDeck packages because it installs the same executable and intentionally uses the same existing OpsDeck data directory. Existing profiles and vaults are preserved.

If RDP is absent in an existing installation, enable **RDP** in the sidebar's modules menu (⊞). New installations enable it by default.

## Profiles and connections

Profiles and groups are stored in `~/.config/opsdeck/rdp.json`; passwords are never serialized there. Groups can be renamed or changed by editing a host. Search covers names, groups, addresses, users and domains. Connections also appear in the command palette.

Authentication options:
- Ask each time: masked password dialog, cancelled connections do nothing; spaces in passwords are preserved.
- OS keyring: credential service `opsdeck`, key `rdp:<profile-id>`; blank input preserves an existing password. Clear-password and profile deletion remove it; switching away from keyring removes the obsolete credential.
- KeePass: backend resolves the selected entry at connect time; no password is returned to the UI. An empty profile username uses the entry's username.

Settings include port, domain, width/height, scale (100/140/180), fullscreen, multiple monitors, dynamic resolution, audio, clipboard and administrative session. Clipboard and audio redirection are off by default. Certificate checking defaults to strict (`/cert:deny`); TOFU trusts the first certificate and ignore disables verification. Both alternatives are explicit profile choices.

Connections use `xfreerdp3 /args-from:stdin`, with one validated option per line, sent through an anonymous pipe and closed after writing. No shell or password in process arguments, environment, temporary files, clipboard or captured child logs. Backend password buffers are zeroized; UI password fields are cleared on close and after save attempts. Control characters in credentials are rejected because newline separates options. Requires FreeRDP 3 with `/args-from:stdin` (verified against installed 3.32.1 and FreeRDP 3.10.3 sources).

The session list provides **Disconnect**. FreeRDP exit codes are reported without raw logs. Closing OpsDeck leaves independent RDP windows running; restart does not adopt those older processes. No remote RDP server was provided, so live Windows login, NLA and server certificate behavior require checking against your hosts.

## Update upstream safely

The fork's **master stays upstream-only**. `feature/rdp` is the default branch so GitHub can run its scheduled personal workflows without putting fork workflows into master.

`rdp-sync.yml` runs daily and manually. `scripts/sync-rdp.sh` checks a clean working tree, fast-forwards the local feature branch from origin, refuses independent fork-master changes, and merges `upstream/master` into `feature/rdp`. Conflicts abort the merge and fail the workflow. An atomic, non-forced push advances master and feature together. Concurrent remote changes reject the push. No PR is opened to upstream.

Local use:

```sh
git switch feature/rdp
scripts/sync-rdp.sh
```

On a conflict, merge and resolve manually, run tests, commit, then push normally. Never force-push the user's branch. Scheduled Actions push with GITHUB_TOKEN, then explicitly dispatch the build because that token's pushes do not trigger another push workflow.

## Builds and signatures

`rdp-arch.yml` is triggered on feature pushes and manual dispatch. It builds a native x86_64 Arch package in the Arch container, runs Rust/unit/browser tests, and optionally builds a signed AppImage on Ubuntu 22.04. Releases are drafted and published only after the required jobs succeed. Versions are derived from upstream plus the workflow run number: `X.Y.Z-rdp.N`; the upstream version files are not changed by release commits. SemVer comparison recognizes RDP build numbers.

Arch packages update through **pacman**, not Tauri's in-app installer. Native builds use `OPSDECK_PACKAGE_FORMAT=arch` and show this guidance in Update Settings. A package's detached `.tauri.sig` is a Tauri/minisign signature, **not** a pacman PGP signature. The distinct suffix prevents pacman from treating it as a PGP signature. SHA256SUMS accompanies release packages. The AppImage is the supported Tauri auto-update path on Linux.

The personal updater uses **only Rewentes/opsdeck Releases** and the fork's own public key. The original author's private key is neither available nor needed. `TAURI_SIGNING_PRIVATE_KEY` is stored in the fork's GitHub Actions Secrets. Its password is empty, so secret storage/file permissions protect it. Keep a private backup of the local generated key outside Git; the initial task checkout keeps it in its sibling `work/signing/rdp-updater.key`, mode 0600. Do not rotate it casually: already installed AppImages trust the existing public key. The public key is embedded in `src-tauri/tauri.conf.json`. Signed AppImages and `.sig` feed the existing latest-json script. Builds without the secret still produce the native package but do not publish a signed updater manifest.

Upstream workflows are not the personal release entry point; use **RDP Arch build** and **Sync upstream into RDP** in the fork.

## Local source build

Install Rust, Node/npm, Arch native dependencies and base-devel. Then:

```sh
npm ci
scripts/build-arch.sh
sudo pacman -U packaging/arch/opsdeck-rdp-*.pkg.tar.zst
```

`makepkg` must run as a normal user. The PKGBUILD packages the locally compiled executable, not a network download; Actions does the compilation in an Arch container before packaging.
