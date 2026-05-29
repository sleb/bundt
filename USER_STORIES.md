# Bundt — User Stories

## Personas

- **Script author** — writes single-file Bun scripts (automation, CLIs, one-offs) without a full project scaffold
- **Project developer** — works in a Bun monorepo or app with a `bun.lockb` but no hand-written `tsconfig.json`
- **Extension maintainer** — maintains the Bundt Zed extension and ships updates

---

## Epic 1: Zero-config Bun context detection

**US-01** — As a script author, when I open a `.ts` file that starts with `#!/usr/bin/env bun`, Bundt activates automatically so I get Bun-aware type checking without touching any config.

**US-02** — As a script author, when I open a `.ts` or `.js` file that imports from the `"bun"` module (e.g. `import { $ } from "bun"`), Bundt activates automatically.

**US-03** — As a project developer, when I open any TypeScript file in a workspace that contains a `bun.lockb`, Bundt activates for the whole workspace.

**US-04** — As a script author, when none of the above signals are present, Bundt stays inactive and Zed falls back to its default TypeScript LSP behaviour, so non-Bun projects are unaffected.

---

## Epic 2: Virtual workspace synthesis

**US-05** — As a script author, when Bundt activates, it synthesises a `tsconfig.json` in memory (no files written to disk) with settings appropriate for Bun (`moduleResolution: bundler`, `target: ESNext`, `lib: [ESNext]`, `types: ["bun-types"]`, `resolveJsonModule: true`) so the LSP understands Bun's module semantics and file import conventions.

**US-06** — As a script author, when Bundt activates for a single-file script that has no `tsconfig.json` on disk, the synthesised config covers exactly that file so diagnostics are accurate and scoped correctly.

**US-07** — As a project developer, when a `tsconfig.json` already exists on disk, Bundt merges or extends it rather than replacing it, so project-specific compiler options are preserved.

**US-08** — As a script author, I do not need to run `bun install` or `npm install` to get `@types/bun` — Bundt ships the type declarations and makes them available automatically.

**US-32** — As a script author, when I write `import data from "./config.json"` in a bun-context file, I get full property-level autocomplete on `data` because the synthesised tsconfig includes `resolveJsonModule: true` and TypeScript infers the shape of the file.

**US-33** — As a script author, when I write `import cfg from "./config.toml"` or `import txt from "./README.txt"`, the import resolves without a TypeScript error because Bundt ensures the `@types/bun` ambient module declarations (which declare `*.toml` and `*.txt` as typed imports) are active. Deep autocomplete on TOML values is not provided — the imported type is the shape declared by `@types/bun`, not a per-file inference.

---

## Epic 3: LSP proxy and feature parity

**US-09** — As a script author, I see red squiggles disappear on `Bun.serve`, `$`, `fetch`, and other Bun globals once the extension activates, because the proxy injects the correct types before the downstream TypeScript language server processes the file.

**US-10** — As a script author, I get accurate auto-complete suggestions for Bun APIs (e.g. `Bun.file`, `Bun.serve`, `Bun.password`) powered by the bundled type declarations.

**US-11** — As a script author, go-to-definition on a Bun API navigates to the bundled type declaration, so I can read the signature without leaving the editor.

**US-12** — As a script author, all standard TypeScript LSP features (completions, hover docs, find-references, rename symbol, inlay hints) continue to work exactly as they do with the default `typescript-language-server` setup, because non-Bun-specific traffic is forwarded transparently.

**US-13** — As a script author, diagnostics update in real time as I edit, with no noticeable additional latency introduced by the proxy layer.

---

## Epic 4: Zed extension integration

**US-14** — As a script author, I install Bundt from the Zed extension marketplace with a single click and no post-install steps.

**US-15** — As a script author, after installing Bundt, I do not need to edit `settings.json`, add workspace config, or restart Zed manually — the extension registers itself for TypeScript/JavaScript files automatically.

**US-16** — As a script author, Bundt does not conflict with other TypeScript extensions; if I disable Bundt, Zed's built-in TypeScript support resumes without residual side-effects.

**US-17** — As an extension maintainer, the entire extension — both the Zed shim and the LSP proxy — is written in Rust, so there is one language, one toolchain, and one `cargo build` step to produce all deliverables.

---

## Epic 5: Bundled dependencies and self-contained distribution

**US-18** — As a script author, `typescript-language-server` is downloaded automatically on first use via `bun x` (Bun's package runner) with no manual install step. After the first activation, the package is cached by Bun and no further network requests are made.

**US-19** — As an extension maintainer, `@types/bun` declarations are vendored inside the extension bundle so the shipped version is known-good and updates are controlled via extension releases.

**US-20** — As a script author, Bundt works out of the box using its bundled `@types/bun` declarations with no configuration required.

**US-20b** _(future)_ — As a project developer, I can configure in Zed settings which version of `@types/bun` Bundt uses: `bundled` (default), a `localPath` pointing to a directory on disk, or `download` to fetch a specific version from the npm registry at activation time.

**US-21** — As an extension maintainer, the proxy binary is compiled for each supported platform (linux-x64, macos-x64, macos-arm64, windows-x64) and shipped as extension assets. Bun is the only runtime dependency, and it is already present on any machine running Bun scripts — no Node or npm install is required.

---

## Epic 6: Bun test runner integration

**US-22** — As a script author, when a file imports from `"bun:test"` (e.g. `import { test, expect } from "bun:test"`), Bundt signals to Zed that the file contains tests so the editor's test runner gutter controls appear.

**US-23** — As a script author, I can run a single test or a single `describe` block directly from the gutter without leaving the editor, and Bundt translates that into the correct `bun test --testNamePattern` invocation.

**US-24** — As a script author, test pass/fail results appear inline in the editor after a run, matching Zed's standard test results UI, so I don't have to read a terminal to see which assertions failed.

**US-25** — As a project developer, running all tests in the workspace from Zed's test panel invokes `bun test` at the workspace root, and results are reported back per-file and per-test.

---

## Epic 7: Reliability and error handling

**US-26** — As a script author, if the downstream TypeScript language server is not found, Bundt surfaces a clear error message in the Zed LSP log rather than silently failing.

**US-27** — As a script author, if the proxy crashes or the underlying TypeScript language server process exits, Zed's standard LSP restart behaviour kicks in and Bundt recovers without requiring a manual editor restart.

**US-28** — As a script author, malformed or oversized JSON-RPC messages from the downstream language server do not crash the proxy — they are logged and skipped gracefully.

---

## Epic 8: Diagnostics and debugging

**US-29** — As a script author, I can run a Zed command (`Bundt: Show Virtual Config`) that prints the synthesised `tsconfig.json` — exactly as it was sent to the downstream TypeScript language server — to the Zed output panel, so I can diagnose unexpected type errors.

**US-30** — As a script author, I can run a Zed command (`Bundt: Show Detection Info`) that prints which signal triggered activation (shebang / `"bun"` import / `bun.lockb`) and the workspace root that was resolved, so I can understand why Bundt did or did not activate.

**US-31** — As an extension maintainer, I can enable verbose mode (via a Zed setting `bundt.logLevel: "debug"`) to write full JSON-RPC message logs to a file, allowing proxy traffic to be inspected when investigating edge-case LSP behaviour without impacting performance in normal use.

---

## Out of scope (v1)

- Support for editors other than Zed
- Modifying or writing any files to the user's project on disk
- Supporting non-TypeScript Bun scripts (`.sh`, plain JS without types)
- Custom `tsconfig.json` UI or settings panel within Zed
- Downloading `@types/bun` at activation time (tracked as US-20b for a future release)
- YAML file imports (Bun has no native YAML import support)
- Deep per-file shape inference for TOML imports (would require parsing TOML at LSP time)
