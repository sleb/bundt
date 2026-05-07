# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`bundt` is a proxy LSP server written in Rust. It sits between an IDE and a TypeScript LSP (`typescript-language-server` by default), detects Bun context, synthesises a virtual `tsconfig.json` in memory, and injects bundled `@types/bun` declarations — then forwards everything else unchanged. The goal is zero-config Bun TypeScript intelligence for single-file scripts.

This repo is the **core proxy binary only**. Editor support is provided by per-IDE extension repos (`bundt-zed` for Zed, with others to follow) that bundle and launch `bundt`.

## Build commands

```
cargo build
cargo test
cargo clippy
cargo build --release
```

## Releasing

See [`RELEASING.md`](RELEASING.md) for the full release process, including the
branch workflow, versioning policy, and pre-tag checklist.

## Architecture

### Two-repository split

| Repo           | Role                                                             |
| -------------- | ---------------------------------------------------------------- |
| `bundt`        | Core LSP proxy binary — pure Rust, no editor dependency          |
| `bundt-zed`    | Zed extension — shim that bundles and launches `bundt`           |
| _(future)_     | VS Code extension, etc. — same pattern as `bundt-zed`            |

Each editor extension declares a minimum required `bundt` version. A `bundt` release that doesn't change the protocol contract doesn't require an extension release.

### Proxy design

`bundt` is a transparent JSON-RPC proxy. All LSP traffic flows through it. For non-Bun files, it forwards unchanged. For Bun files, it intercepts `initialize` / `textDocument/didOpen` to inject the synthesised config and type declarations before forwarding.

**Detection signals (in priority order):**

1. `#!/usr/bin/env bun` shebang in the file
2. `import … from "bun"` statement
3. `bun.lockb` present in the workspace root

**Virtual tsconfig synthesis** happens in memory — nothing is written to disk. Key fields: `moduleResolution: "bundler"`, `resolveJsonModule: true`, plus ambient declarations for TOML and text file imports. If an on-disk `tsconfig.json` exists, it is merged/extended rather than replaced.

**Bundled `@types/bun`** is vendored inside the binary at a pinned version. No `bun install` or network access required.

### Release milestones

- **v0.1** — Transparent proxy. Zero Bun behaviour; establishes that substituting `bundt` for the downstream TS LSP introduces no regressions.
- **v0.2** — MVP. Bun detection + virtual tsconfig synthesis + `@types/bun` injection.
- **v0.3** — Debug protocol: custom LSP commands that return the synthesised tsconfig and detection signal, plus verbose JSON-RPC logging.

See `ARCHITECTURE.md` for component contracts and invariants, `ROADMAP.md` for the full story-level breakdown, and `USER_STORIES.md` for the complete requirements.

### Platform targets

Linux x64, macOS x64, macOS arm64, Windows x64. Binaries are shipped as extension assets in `bundt-zed`.

## Documentation

`ROADMAP.md` and `USER_STORIES.md` are the source of truth for intended behaviour. If implementation diverges from them — a design decision changes, a story is descoped, an API differs from what's written — update the docs in the same commit. Stale docs are worse than no docs.

## Testing

Tests should cover meaningful behaviour: edge cases, non-obvious invariants, failure paths. Don't write tests to pad coverage or to verify that Rust's standard library or the LSP crate does what it says. No duplicate tests for the same logical case. If a test doesn't fail for a reason you can articulate, delete it.

## Fearless Refactoring

Minimize or avoid backward-compatible fallback logic:

- If a file is expected to exist, read it and propagate the error — don't
  silently skip
- If a config value is required, fail loudly on startup — don't substitute a
  default that hides a misconfiguration
- If a type or function is renamed or removed, delete the old name — no
  re-exports, no aliases, no deprecation shims
- When changing a data structure or interface, update all call sites immediately
  rather than keeping old paths alive
