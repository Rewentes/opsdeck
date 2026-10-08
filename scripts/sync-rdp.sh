#!/usr/bin/env bash
# Requires a clean checkout of feature/rdp. Never rebases, force-pushes or resolves conflicts automatically.
set -euo pipefail
[ "$(git branch --show-current)" = feature/rdp ] || { echo 'Checkout feature/rdp first'; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo 'Working tree has changes; sync stopped'; exit 1; }
git remote get-url upstream >/dev/null 2>&1 || git remote add upstream https://github.com/LeoAlecksey/opsdeck.git
git fetch upstream master
git fetch origin master feature/rdp
git merge --ff-only origin/feature/rdp
git merge-base --is-ancestor origin/master upstream/master || { echo 'Fork master contains independent changes; sync stopped'; exit 1; }
if ! git merge --no-edit upstream/master; then
  git merge --abort
  echo 'Upstream conflicts with RDP. Resolve manually; nothing was pushed.'
  exit 1
fi
# Atomic, non-forced updates: concurrent edits reject both pushes.
git push --atomic origin upstream/master:master HEAD:feature/rdp
