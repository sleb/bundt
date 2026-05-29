# Changelog

All notable changes to this project will be documented in this file.

## [v0.2.0] - 2026-05-28

### Added

- **Bun context detection** — three signals evaluated in priority order:
  - `#!/usr/bin/env bun` shebang on the first line (US-01)
  - Static ES module import from `"bun"` (`import … from "bun"`, `import "bun"`, `export … from "bun"`) (US-02)
  - `bun.lockb` present in the workspace root (US-03)
  - Non-Bun files pass through to the downstream LSP unchanged (US-04)
- **Bundled `@types/bun`** — declaration files for `bun-types` 1.3.14 are embedded in the binary at compile time; no `bun install` or network access needed (US-08, US-19)
- **Virtual tsconfig synthesis** — on first Bun activation, bundt injects a `workspace/didChangeConfiguration` notification to the downstream TS LSP setting `implicitProjectConfig.compilerOptions` to `{ moduleResolution: "bundler", target: "ESNext", lib: ["ESNext"], resolveJsonModule: true, typeRoots: […], types: ["bun-types"] }` (US-05, US-06, US-32, US-33)
- **Type extraction** — `@types/bun` declaration files are extracted to `{data_dir}/bundt/bun-types-{VERSION}/bun-types/` on first activation; subsequent sessions skip extraction if the directory exists (US-08)
- **Router interception** — `initialize` captures workspace root and checks for `bun.lockb`; `initialized` triggers configuration injection for lockfile workspaces; `textDocument/didOpen` triggers detection and configuration injection for shebang/import files; `textDocument/didClose` evicts the URI from the detection cache

### Not included in this release

- US-07: merging with an existing on-disk `tsconfig.json` — deferred to v0.3+. In v0.2, Bun intelligence applies to implicit projects (files not covered by a `tsconfig.json`) only.

## [v0.1.0] - 2026-05-04

### Added

- Transparent JSON-RPC proxy: `bundt <lsp-binary> [args…]` sits between the IDE and a downstream TypeScript LSP, forwarding all LSP traffic unchanged (US-12).
- LSP Content-Length frame codec (`framing` module): reads and writes the standard `Content-Length: N\r\n\r\n` envelope used by all LSP implementations.
- Bidirectional forwarding loop: two concurrent tasks relay IDE → LSP and LSP → IDE independently, so neither direction can stall the other.
- Graceful shutdown: when either side closes its connection the proxy drains in-flight frames and exits with the same code as the downstream LSP (US-27).
- Malformed JSON-RPC frames are logged and skipped rather than crashing the proxy (US-28).
- Clear error message when the downstream LSP binary is not found (US-26).
- Platform binaries: Linux x86_64, macOS x86_64, macOS aarch64, Windows x86_64.
