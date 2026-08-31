#!/usr/bin/env bash
# Publish the mssql-rust.github.io subproject to its sibling read-only
# export repo, via git subtree. Per spec/monorepo-github-pages/index.md:
#
#   - Always maintain the GitHub Pages subproject in this monorepo, at
#     mssql-rust.github.io/. Never edit the sibling export directly.
#   - The sibling export lives at github.com/mssql-rust/mssql-rust.github.io
#     (a real GitHub Pages user/org-site repo — GitHub Pages needs a repo
#     literally named "<org>.github.io"), and, if cloned locally, at
#     ~/git/mssql-rust/mssql-rust.github.io (a sibling of this checkout,
#     not nested inside it).
#
# This script runs a `git subtree push`, which is the split-then-push this
# describes in one step: it rewrites mssql-rust.github.io/'s history so
# paths are relative to that subdirectory's own root (so its
# .github/workflows/deploy.yml lands where GitHub Actions can see it) and
# pushes the result as the export repo's `main`.
#
# Run from anywhere in the working tree:  bin/publish-pages.sh
#
# `git subtree push` is a plain (non-force) push under the hood, so it
# refuses to overwrite the export with older content — run it from a
# checkout whose mssql-rust.github.io/ is at least as current as what's
# already published, or this fails with "non-fast-forward" rather than
# silently rolling the live site back.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

remote_url="git@github.com:mssql-rust/mssql-rust.github.io.git"
remote_name="pages"

if ! git remote get-url "$remote_name" >/dev/null 2>&1; then
	git remote add "$remote_name" "$remote_url"
fi

git fetch "$remote_name" main
git subtree push --prefix=mssql-rust.github.io "$remote_name" main
