# Releasing bundt

---

## Workflow (GitHub flow)

`main` is always the latest **released** state. Every commit on `main` corresponds to a tagged release. Work-in-progress never lands on `main` until the release is ready to ship.

### Branches

| Branch pattern      | Purpose                                                           |
| ------------------- | ----------------------------------------------------------------- |
| `main`              | Released code only — never commit work-in-progress here          |
| `feat/<short-name>` | A feature, story, or doc change (e.g. `feat/us-05-synthesiser`)  |
| `fix/<short-name>`  | A bug fix or patch (e.g. `fix/lockfile-detection-on-windows`)    |

Cut every branch from `main`:

```bash
git checkout main
git pull
git checkout -b feat/us-05-synthesiser
```

### Merging

Merge to `main` as the **last step of releasing**, not before. The flow is:

1. Do all work on the feature branch.
2. Run the full release checklist (below) while still on the branch.
3. When everything passes, merge to `main` and tag immediately.

```bash
git checkout main
git merge --no-ff feat/us-05-synthesiser -m "Release v0.2.0"
git tag -a v0.2.0 -m "v0.2.0"
git push && git push --tags
```

The `--no-ff` flag keeps the merge commit even when a fast-forward is possible, so the graph shows clearly where each release landed.

Pushing the tag triggers the GitHub Actions release workflow, which builds platform binaries and creates the GitHub release automatically — see [`.github/workflows/release.yml`](.github/workflows/release.yml).

### Patches

For a patch release, cut the branch from the relevant tag rather than from the current tip of `main`:

```bash
git checkout -b fix/lockfile-detection-on-windows v0.2.0
# fix, test, then merge back to main and tag v0.2.1
```

### What goes straight to `main`

Typo fixes in docs that don't touch any behaviour can go directly to `main`. When in doubt, use a branch.

---

## Versioning

`bundt` follows [Semantic Versioning 2.0.0](https://semver.org/).

| Increment | When                                                                    |
| --------- | ----------------------------------------------------------------------- |
| `PATCH`   | Bug fixes that don't add or change behaviour                            |
| `MINOR`   | A roadmap milestone is complete                                         |
| `MAJOR`   | Breaking change to the LSP protocol contract or CLI interface           |

Before `1.0.0`, minor version bumps may include breaking changes — standard pre-1.0 semver practice. Each roadmap milestone maps to one minor release (`v0.1`, `v0.2`, …). Patch releases may be cut between milestones for critical bug fixes.

### Coordinating with bundt-zed

Each `bundt-zed` release declares a minimum required `bundt` version. A `bundt` release that doesn't change the protocol contract doesn't require a `bundt-zed` release. When a `bundt` release does change the contract, open a tracking issue in `bundt-zed` before tagging.

---

## Release checklist

Work through these in order. Every item must pass before tagging.

### 1. Verify the implementation is complete

- [ ] All steps in `design/v{VERSION}-plan.md` are marked ✅ Done
- [ ] All user stories for the milestone are implemented (cross-check `ROADMAP.md`)

### 2. Verify docs are in sync with the code

Drift accumulates during development; the release is the forcing function to clear it. Check each of these against the source:

- [ ] **`ARCHITECTURE.md`** — component descriptions, message flow diagram, contracts, invariants
- [ ] **`design/router.md`** — lifecycle, concurrency model, interception table, error handling
- [ ] **`design/detector.md`** _(v0.2+)_ — interface types, signal evaluation rules, error handling
- [ ] **`design/synthesiser.md`** _(v0.2+)_ — interface types, required/additive fields, merge algorithm, bundled version

Fix any drift before continuing. These edits belong in their own commit or can be squashed into the release commit if trivial.

### 3. Quality gates

```bash
cargo test
cargo clippy -- -D warnings
```

All tests must pass. Zero Clippy warnings.

### 4. Update version and docs

- [ ] **`Cargo.toml`** — bump `version` to the new version string
- [ ] **`CHANGELOG.md`** — add a new `## [vX.Y.Z] - YYYY-MM-DD` section; this is the text that appears verbatim in the GitHub release notes
- [ ] **`ROADMAP.md`** — add the release date to the completed milestone heading (e.g. `## v0.2 — MVP _(released YYYY-MM-DD)_`)
- [ ] **`design/v{VERSION}-plan.md`** — confirm all steps show ✅ Done

### 5. Commit

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md ROADMAP.md design/v{VERSION}-plan.md
# plus any docs changed in step 2
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

Pushing the tag kicks off the release workflow. Within a few minutes the GitHub release page will have platform binaries attached and release notes populated from `CHANGELOG.md`.

### 8. Build platform binaries

Build release binaries for all supported targets:

```bash
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-pc-windows-msvc
```

Exact cross-compilation setup is specified in `design/v{N}/plan.md` for the first release that ships binaries.

### 9. Create the GitHub release

```bash
gh release create v{VERSION} \
  --title "v{VERSION}" \
  --notes-file /tmp/release-notes.md \
  target/x86_64-unknown-linux-gnu/release/bundt \
  target/x86_64-apple-darwin/release/bundt \
  target/aarch64-apple-darwin/release/bundt \
  target/x86_64-pc-windows-msvc/release/bundt.exe
```

**Release notes format:** use the roadmap milestone table as the starting point — list the user stories shipped, note any design decisions or caveats, and call out any minimum version requirement for `bundt-zed`.

---

## After the release

- [ ] Verify the GitHub release page looks correct (tag, assets, notes, not a draft)
- [ ] If the protocol contract changed, open a tracking issue in `bundt-zed` to update its minimum `bundt` version
- [ ] Open the next milestone in `ROADMAP.md` — create `design/v{NEXT_VERSION}-plan.md` if it doesn't already exist
