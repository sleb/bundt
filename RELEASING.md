# Releasing bundt

---

## Workflow (GitHub flow)

`main` is always the latest **released** state. Every commit on `main`
corresponds to a tagged release. In-progress work lives on a feature branch
until it is ready to ship.

### Branches

| Branch pattern      | Purpose                                                      |
| ------------------- | ------------------------------------------------------------ |
| `main`              | Released code only — never commit work-in-progress here      |
| `feat/<short-name>` | A feature, story, or doc change (e.g. `feat/v0.2-detection`) |
| `fix/<short-name>`  | A bug fix or patch (e.g. `fix/framing-eof-mid-header`)       |

Cut every branch from `main`:

```bash
git checkout main
git pull
git checkout -b feat/v0.2-detection
```

### Merging

Merge to `main` as the **last step of releasing**, not before. The flow is:

1. Do all work on the feature branch.
2. Run the full release checklist (below) while still on the branch.
3. When everything passes, merge to `main` and tag immediately.

```bash
git checkout main
git merge --no-ff feat/v0.2-detection -m "Release v0.2.0"
git tag -a v0.2.0 -m "v0.2.0"
git push && git push --tags
```

The `--no-ff` flag keeps the merge commit even when a fast-forward is possible,
so the graph shows clearly where each milestone landed.

Pushing the tag triggers the GitHub Actions release workflow, which builds
platform binaries and creates the GitHub release automatically — see
[`.github/workflows/release.yml`](.github/workflows/release.yml).

### Patches

For a patch release, cut the branch from the relevant tag rather than from
the current tip of `main`:

```bash
git checkout -b fix/framing-eof-crash v0.2.0
# fix, test, then merge back to main and tag v0.2.1
```

### What goes straight to `main`

Typo fixes in docs that don't touch any code or design documents can go
directly to `main`. When in doubt, use a branch.

---

## Versioning

bundt follows [Semantic Versioning 2.0.0](https://semver.org/). Given a version
`MAJOR.MINOR.PATCH`:

| Increment | When                                                              |
| --------- | ----------------------------------------------------------------- |
| `PATCH`   | Bug fixes that don't add or change behaviour                      |
| `MINOR`   | A roadmap milestone is complete (new proxy capabilities shipped)  |
| `MAJOR`   | Breaking change to the CLI interface or the LSP protocol contract |

Before `1.0.0`, minor version bumps may include breaking changes — this is
standard pre-1.0 practice under semver. Each roadmap milestone maps to one
minor release (`v0.1`, `v0.2`, …). Patch releases may be cut between
milestones for critical bug fixes.

---

## Release checklist

Work through these in order. Every item must pass before tagging.

### 1. Verify the implementation is complete

- [ ] All steps in `design/v{N}.md` are marked ✅ Done
- [ ] All user stories for the milestone are implemented (cross-check `ROADMAP.md`)

### 2. Verify docs are in sync with the code

Architecture and component docs must accurately reflect the current
implementation before a release is tagged. Check each of these against the
source:

- [ ] **`ARCHITECTURE.md`** — component diagram, message-flow sequences,
      layer contracts, and invariants
- [ ] **`design/router.md`** — lifecycle, concurrency model, message
      interception table, and error-handling notes
- [ ] **`design/detector.md`** _(v0.2+)_ — detection signals, priority order,
      `DetectionResult` type, and all listed invariants
- [ ] **`design/synthesiser.md`** _(v0.2+)_ — required overrides, merge
      strategy, virtual URI scheme, and invariants

Fix any drift before continuing. These edits belong in their own commit (or
can be squashed into the release commit if trivial).

### 3. Quality gates

```bash
cargo test                    # all tests pass
cargo clippy -- -D warnings   # zero warnings
```

### 4. Update version and docs

- [ ] **`Cargo.toml`** — bump `version` to the new version string
- [ ] **`CHANGELOG.md`** — add a new `## [vX.Y.Z] - YYYY-MM-DD` section above
      the previous one; this is the text that appears verbatim in the GitHub
      release notes (see [`CHANGELOG.md`](CHANGELOG.md) for format)
- [ ] **`README.md`** — update the feature list to reflect only what is
      actually shipped in this release; remove anything still in a future
      milestone
- [ ] **`ROADMAP.md`** — add the release date to the completed milestone
      heading (e.g. `## v0.1 — Transparent proxy _(released 2026-05-02)_`)

### 5. Commit

Stage only the files changed in steps 2–4:

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md README.md ROADMAP.md
# plus any ARCHITECTURE.md or design/*.md changed in step 2
git commit -m "Release v{VERSION}"
```

### 6. Merge to `main` and tag

```bash
git checkout main
git merge --no-ff feat/<branch> -m "Release v{VERSION}"
git tag -a v{VERSION} -m "v{VERSION}"
```

### 7. Push

```bash
git push && git push --tags
```

Pushing the tag kicks off the release workflow. Within a few minutes the
GitHub release page will have platform binaries attached and release notes
populated from `CHANGELOG.md`.
