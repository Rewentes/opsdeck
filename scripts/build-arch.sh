#!/usr/bin/env bash
# Run from the repository root as a normal user on Arch, after npm ci.
set -euo pipefail
export OPSDECK_PACKAGE_FORMAT=arch
npm run build
cargo build --release --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml --locked
install -m755 src-tauri/target/release/opsdeck packaging/arch/opsdeck
install -m644 src-tauri/icons/128x128.png packaging/arch/opsdeck.png
install -m644 LICENSE packaging/arch/LICENSE
(cd packaging/arch && makepkg --nodeps --force)
