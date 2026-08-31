# Repository-level tasks that don't belong to any one family's own tooling
# (cargo covers the Rust crate; npm covers the site's own build/dev tasks —
# this covers cross-cutting operations on the monorepo as a whole).

.PHONY: github-pages

# Publish mssql-rust.github.io/ (a monorepo subtree, spec/monorepo-github-pages/)
# to the standalone, read-only GitHub Pages export repo. Delegates to
# bin/make-github-pages (a standalone POSIX script, not inlined here) so it
# can be run and read on its own like any other bin/ script.
github-pages:
	bin/make-github-pages
