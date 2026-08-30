# llms.json and llms.txt

Create AI guidance helper files at the repo root:

- `llms.json` -> JSON
- `llms.txt` -> markdown text

Purpose: Provide AI tools with a clean, curated map of its most important content.

Help large language models (LLMs) read, understand, and cite a site's documentation or resources without getting bogged down 

File size:  < 40k bytes.

## Caveat: don't blindly copy across contexts

A workspace/crate-root `llms.txt`/`llms.json` and a deployed website's
`llms.txt`/`llms.json` are not the same document, even when they describe
the same project. The crate-root version's links point at wherever the
crate's own files resolve (typically the GitHub repo, docs.rs, crates.io);
serving that exact text unmodified from the website's `llms.txt` carries
those same GitHub/docs.rs links along, plus at least one link that should
have pointed back at a sibling file on the site itself (e.g. an `llms.json`
cross-reference) but instead points at the GitHub copy — technically still
resolvable, but wrong for a document whose whole point is "the map of this
site."

When serving from a deployed site, build a website-appropriate version
instead of copying verbatim: point each entry at wherever it actually
resolves *from the site's own domain* — the site's own pages for content
the site hosts, and out to GitHub/docs.rs/crates.io only for content the
site doesn't host itself (README, CHANGELOG, source examples, etc. usually
live only in the repo). Cross-references between the two files (e.g.
`llms.txt` mentioning `llms.json`) should resolve on that same domain, not
loop back to the other context's copy.
