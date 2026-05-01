# bundt

> **bun** + TypeScript's **d.ts** declaration files.

A proxy LSP that gives Bun scripts full TypeScript intelligence — no per-project scaffolding required.

Open a `.ts` file with a `#!/usr/bin/env bun` shebang or a `import { $ } from "bun"` and it just works: completions, hover docs, go-to-definition, no red squiggles.

## How it works

`bundt` sits between your IDE and `vtsls`. It detects bun context, synthesises a virtual `tsconfig.json` in memory, and injects bundled `@types/bun` declarations — then forwards everything else to `vtsls` unchanged.

## Repositories

| Repo        | Purpose                                      |
| ----------- | -------------------------------------------- |
| `bundt`     | Core LSP proxy binary (this repo)            |
| `bundt-zed` | Zed extension — bundles and launches `bundt` |

## Docs

- [User stories](USER_STORIES.md) — what the system does and for whom
- [Roadmap](ROADMAP.md) — planned releases for `bundt` and `bundt-zed`
