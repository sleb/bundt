# Synthesiser

Produces the in-memory TypeScript configuration and type declarations that give a Bun-context file full type intelligence.

---

## Interface

**Inputs**

| Field               | Type                        | Description                                                                              |
| ------------------- | --------------------------- | ---------------------------------------------------------------------------------------- |
| `signal`            | `Signal`                    | The triggering signal from the Detector                                                  |
| `existing_tsconfig` | `Option<serde_json::Value>` | Pre-parsed content of the on-disk `tsconfig.json`, if present; the caller reads the file |

The Synthesiser does not read from disk itself. The caller (Router) is responsible for locating and reading any on-disk `tsconfig.json` before calling the Synthesiser.

**Output**

```
SynthesisResult {
  tsconfig: serde_json::Value,   // merged, in-memory tsconfig
  types_root: VirtualUri,        // prefix under which @types/bun declarations are served
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

### Merge algorithm

1. Start from `existing_tsconfig` if present, otherwise start from `{}`.
2. Ensure `compilerOptions` object exists.
3. Apply required overrides (see above).
4. Apply additive merges (see above).

The output `tsconfig` is a complete JSON object suitable for passing to the downstream TS LSP. Nothing is written to disk.

---

## @types/bun declarations

The `@types/bun` declaration files are **embedded in the `bundt` binary at compile time**. They are not read from disk, not fetched from the network, and not resolved through `node_modules`.

The embedded version is pinned at the time of the `bundt` release. Updating the bundled version requires a new `bundt` release.

The Synthesiser exposes the declarations to the downstream TS LSP via a stable virtual URI prefix (`types_root`). The exact URI scheme and the LSP-level mechanism for making the declarations visible to vtsls are specified in the v0.2 release design.

The `@types/bun` package includes ambient module declarations that cover:

- `*.toml` imports — typed as per the `@types/bun` declaration
- `*.txt` and other text file imports — typed as per the `@types/bun` declaration

Deep per-file shape inference for TOML (e.g. property-level autocomplete from a specific `.toml` file's content) is out of scope.

---

## Invariants

- Only called when `DetectionResult` is `Active`. The Router must not call the Synthesiser for inactive files.
- Never writes to disk.
- Output is deterministic for a given `(signal, existing_tsconfig)` pair.
- The embedded `@types/bun` bytes are compile-time constants; they do not change at runtime regardless of configuration.
