# AI statement

This project is maintained with substantial use of AI coding tools,
primarily [Claude Code](https://claude.com/claude-code). This statement says
plainly what that means in practice, for both subprojects in this monorepo.

## What AI tools do here

AI agents research and triage upstream Tiberius PRs and issues, write and
review code, run the test suite, prepare releases, and — as of
2026-09-02 — publish the crate to crates.io. See [`AGENTS.md`](AGENTS.md)
for how an agent is expected to work in this repository, and
[`mssql-rust-maintainer-skill/`](mssql-rust-maintainer-skill/) for the
maintenance playbook they follow. None of this is unsupervised: a human
reviews and is accountable for what gets merged and published.

## Authorship stays human

Every commit's git `author` and `committer` identity is a human — the
project's maintainer, or a contributor submitting their own PR. A tool is
never the recorded author, never a co-author in the legal/copyright sense,
and never the signer of anything here (e.g. a DCO `Signed-off-by:`). That
responsibility isn't delegated to a tool.

## Disclosure

Policy: keep `Co-Authored-By:` trailers on AI-assisted commits. When an AI
tool did substantial work on a commit, that's disclosed in the commit's own
text — the description says so, and where the tool made the commit itself,
it adds its own `Co-Authored-By:` git trailer naming itself. A commit
written with heavy AI assistance looks like:

```
fix: correct off-by-one in packet length parsing

<body, describing the change>

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
```

### Trailer vs. authorship — the actual distinction

These are two different things, and only one of them is regulated by
"[authorship stays human](#authorship-stays-human)" above:

- **`author` / `committer`** — the git-object-level identity every clone of
  this repo carries forever. This is the thing that means something
  legally (copyright) and procedurally (a DCO `Signed-off-by:`, if this
  project ever adopts one, certifies facts about *this* identity). It is
  always, and only ever, a human's.
- **`Co-Authored-By:`** — free-text in the commit body. GitHub renders it,
  `git log` shows it, but it changes nothing about the commit object's
  actual author or committer. It's the same convention used for human
  pair-programming, borrowed here for honest disclosure: it names the tool
  without claiming the tool holds the `author`/`committer` role, or that
  it's a co-author in the legal sense, or a signer of anything.

Naming a tool in a `Co-Authored-By:` trailer is disclosure, not a
delegation of authorship — so it doesn't conflict with the rule above.

See [`mssql/CONTRIBUTING.md`](mssql/CONTRIBUTING.md#using-ai-tools) for what
this means for a contribution you submit yourself.
