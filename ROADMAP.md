# Bundt — Release Roadmap

## Repository structure

The project lives in two top-level repositories with independent release cycles:

| Repo        | Contents                                                    |
| ----------- | ----------------------------------------------------------- |
| `bundt`     | Core LSP proxy binary — pure Rust, no Zed dependency        |
| `bundt-zed` | Zed extension — Rust shim that bundles and launches `bundt` |

`bundt-zed` releases declare a minimum required `bundt` version. Bumping the core proxy does not require a `bundt-zed` release unless the protocol contract changes.

---

## `bundt` — Core LSP proxy

### v0.1 — Transparent proxy _(released 2026-05-04)_

`bundt` forwards all JSON-RPC traffic to the downstream TypeScript language server without modification. No bun-specific behaviour. Establishes that the proxy architecture is sound and introduces zero regressions against the downstream LSP.

| Story | Summary                                                           |
| ----- | ----------------------------------------------------------------- |
| US-12 | Full LSP feature parity via transparent forwarding                |
| US-13 | No measurable latency added by the proxy layer                    |
| US-26 | Clear error when the downstream TypeScript language server is not found |
| US-27 | Recovery via Zed's standard LSP restart if proxy or downstream LSP crashes |
| US-28 | Malformed JSON-RPC messages are logged and skipped, not fatal     |

**Done when:** substituting `bundt` for `typescript-language-server` in any LSP client produces identical editor behaviour.

---

### v0.2 — Bun context and types _(MVP)_

The core value proposition. `bundt` detects bun context, synthesises a virtual tsconfig in memory, and injects bundled `@types/bun` declarations. A user who wires `bundt` up to any LSP client gets full Bun API intelligence with no per-project config.

| Story | Summary                                                                                    |
| ----- | ------------------------------------------------------------------------------------------ |
| US-01 | Activate on `#!/usr/bin/env bun` shebang                                                   |
| US-02 | Activate on `import … from "bun"`                                                          |
| US-03 | Activate on `bun.lockb` in workspace                                                       |
| US-04 | Stay inactive (fall through to the downstream LSP) when no bun signal present              |
| US-05 | Synthesise virtual tsconfig (`moduleResolution: bundler`, `resolveJsonModule: true`, etc.) |
| US-06 | Single-file script with no tsconfig on disk is covered correctly                           |
| US-07 | Existing on-disk tsconfig is merged/extended, not replaced                                 |
| US-08 | Bundled `@types/bun` — no `bun install` needed                                             |
| US-09 | No squiggles on `Bun.serve`, `$`, Bun globals                                              |
| US-10 | Accurate autocomplete for Bun APIs                                                         |
| US-11 | Go-to-definition navigates to bundled type declarations                                    |
| US-19 | `@types/bun` vendored inside the bundle at a known-good version                            |
| US-32 | JSON imports give property-level autocomplete via `resolveJsonModule`                      |
| US-33 | TOML and text file imports resolve without error via `@types/bun` ambient declarations     |

**Done when:** opening a bare `.ts` file with a bun shebang or `import { $ } from "bun"` produces accurate completions, no squiggles on Bun globals, and working go-to-definition — with no `tsconfig.json` or `node_modules` on disk.

---

### v0.3 — Debug protocol

Implements custom LSP commands that expose the proxy's internal state. The Zed extension (`bundt-zed` v0.3) surfaces these as editor commands; other clients can invoke them directly.

| Story | Summary                                                                         |
| ----- | ------------------------------------------------------------------------------- |
| US-29 | Custom LSP command returns the synthesised tsconfig as sent to the downstream LSP |
| US-30 | Custom LSP command returns the detection signal and resolved workspace root     |
| US-31 | `bundt.logLevel: "debug"` setting enables full JSON-RPC traffic logging to file |

**Done when:** a developer experiencing unexpected LSP behaviour can retrieve the virtual config and activation reason without reading source code.

---

## `bundt-zed` — Zed extension

### v0.1 — Install and go _(requires `bundt` ≥ v0.2)_

One-click install from the Zed marketplace. The extension bundles the correct `bundt` binary for the user's platform, registers for TypeScript/JavaScript files, and requires zero post-install configuration.

| Story | Summary                                                                                        |
| ----- | ---------------------------------------------------------------------------------------------- |
| US-14 | Single-click install from Zed marketplace                                                      |
| US-15 | No post-install config — registers automatically                                               |
| US-16 | No conflict with other TS extensions; clean fallback on disable                                |
| US-17 | Entire extension in Rust — one language, one toolchain                                         |
| US-18 | Fully offline after first activation (first activation downloads `typescript-language-server` via Zed's npm integration) |
| US-20 | Works out of the box with bundled `@types/bun`                                                 |
| US-21 | Platform binaries shipped as extension assets (linux-x64, macos-x64, macos-arm64, windows-x64) |

**Done when:** a user installs the extension, opens a bun script, and gets working type intelligence with no additional setup.

---

### v0.2 — Bun test runner _(requires `bundt` ≥ v0.2)_

Integrates with Zed's test runner UI. Detects `"bun:test"` imports, shows gutter run controls per test and describe block, runs them via `bun test --testNamePattern`, and reports results inline.

| Story | Summary                                                                            |
| ----- | ---------------------------------------------------------------------------------- |
| US-22 | `"bun:test"` import triggers Zed test runner gutter controls                       |
| US-23 | Gutter control runs a single test or describe block                                |
| US-24 | Pass/fail results appear inline in the editor                                      |
| US-25 | Workspace test panel invokes `bun test` at the root; results per-file and per-test |

**Done when:** a user can click a gutter icon next to a `test(…)` call, see it run, and get a green or red result inline without touching the terminal.

---

### v0.3 — Debug UX _(requires `bundt` ≥ v0.3)_

Surfaces the debug protocol from `bundt` v0.3 as Zed command-palette commands and a settings key.

| Story | Summary                                                                          |
| ----- | -------------------------------------------------------------------------------- |
| US-29 | `Bundt: Show Virtual Config` command prints synthesised tsconfig to output panel |
| US-30 | `Bundt: Show Detection Info` command prints activation signal and workspace root |
| US-31 | `bundt.logLevel` Zed setting enables verbose JSON-RPC logging                    |

**Done when:** a user experiencing a problem can open the command palette, run a Bundt command, and immediately see the relevant diagnostic output.

---

## Future / unscheduled

| Story  | Summary                                                                          |
| ------ | -------------------------------------------------------------------------------- |
| US-20b | Configurable `@types/bun` source: bundled (default), local path, or npm download |
| —      | TOML deep-shape autocomplete (requires per-file TOML parsing in the proxy)       |
| —      | Editor support beyond Zed via a generic LSP client adapter                       |
