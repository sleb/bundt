# Synthesiser

Produces the in-memory TypeScript configuration and type declarations that give a Bun-context file full type intelligence.

---

## Interface

**Inputs**

| Field    | Type     | Description                              |
| -------- | -------- | ---------------------------------------- |
| `signal` | `Signal` | The triggering signal from the Detector  |

Merging with an on-disk `tsconfig.json` is deferred to v0.3+. In v0.2 the Synthesiser always starts from `{}`.

**Output**

```
SynthesisResult {
  tsconfig: serde_json::Value,   // in-memory compiler options
  types_root: PathBuf,           // {data_dir}/bundt/bun-types-{VERSION}/ — the typeRoots entry
}
```

---

## tsconfig synthesis

### Bun-required fields

These are always set, overriding whatever the on-disk config contains:

```json
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "target": "ESNext",
    "lib": ["ESNext"],
    "resolveJsonModule": true
  }
}
```

These fields are load-bearing for Bun semantics. A project that has set, for example, `"moduleResolution": "node"` on disk gets it overridden — the override is intentional and logged at debug level.

### Bun-additive fields

These are merged with on-disk values rather than replacing them:

| Field                   | Behaviour                                                                                                                                                  |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `compilerOptions.types` | `"bun-types"` is appended to the existing array. If the array is absent, it is created as `["bun-types"]`. If `"bun-types"` is already present, no change. |

### Merge algorithm (v0.2)

Start from `{}`. Apply all required and additive fields. No on-disk `tsconfig.json` is read or merged — that is deferred to v0.3+.

The output `tsconfig` contains only `compilerOptions` fields and is passed to the downstream TS LSP via `workspace/didChangeConfiguration` → `settings.typescript.tsserver.implicitProjectConfig.compilerOptions`. Nothing is written to disk in the workspace.

---

## @types/bun declarations

The `@types/bun` declaration files are **embedded in the `bundt` binary at compile time** via `build.rs` and `include_bytes!`. They are not read from disk, not fetched from the network, and not resolved through `node_modules`.

The embedded version is pinned at the time of the `bundt` release (v0.2.0 ships `bun-types` 1.3.14). Updating the bundled version requires a new `bundt` release.

### Extraction

On first Bun activation the Synthesiser extracts the embedded files to the user data directory:

```
{data_dir}/bundt/bun-types-{VERSION}/
  bun-types/
    index.d.ts
    bun.d.ts
    … (mirrors vendor/bun-types/ structure)
```

| Platform | `data_dir`                        |
| -------- | --------------------------------- |
| macOS    | `~/Library/Application Support`   |
| Linux    | `$XDG_DATA_HOME` or `~/.local/share` |
| Windows  | `%APPDATA%`                       |

The version string in the path acts as a cache key — if the directory already exists, extraction is skipped. A new `bundt` release with a new `bun-types` version creates a new directory; the old one is left in place.

The synthesised tsconfig sets `typeRoots = ["{data_dir}/bundt/bun-types-{VERSION}"]` and `types = ["bun-types"]`. The downstream TS LSP resolves `bun-types` from the extracted directory.

The `@types/bun` package includes ambient module declarations that cover:

- `*.toml` imports — typed as per the `@types/bun` declaration
- `*.txt` and other text file imports — typed as per the `@types/bun` declaration

Deep per-file shape inference for TOML (e.g. property-level autocomplete from a specific `.toml` file's content) is out of scope.

---

## Invariants

- Only called when `DetectionResult` is `Active`. The Router must not call the Synthesiser for inactive files.
- Never writes to the user's workspace. Extraction targets the OS user data directory, not the project directory.
- Output is deterministic for a given `signal` (the signal value does not affect the output in v0.2).
- The embedded `@types/bun` bytes are compile-time constants; they do not change at runtime regardless of configuration.
