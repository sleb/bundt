# Router

The central message loop. Connects the IDE and the downstream TS LSP subprocess, intercepting and augmenting traffic for Bun-context files.

---

## Lifecycle

```
bundt <ts-lsp-binary> [ts-lsp-args…]
```

1. **Startup** — `bundt` is launched by the editor extension. The TS LSP binary path (and any passthrough args) are provided as CLI arguments. The Router spawns the TS LSP subprocess before entering the message loop.
2. **Message loop** — Reads JSON-RPC frames concurrently from two sources: IDE stdin and TS LSP stdout. Frames are processed or forwarded according to the interception table below.
3. **Shutdown** — On receiving an `exit` notification from the IDE, or on IDE stdin EOF, forward `exit` to the TS LSP, wait for it to terminate, then exit `bundt` with code 0.

---

## Message interception

### IDE → TS LSP

| Message                                                       | Action                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `initialize` (request)                                        | Check `workspace_root` (from `rootUri` / `rootPath` params) for `bun.lockb`. If present, record workspace-level activation. Forward the request, augmenting `initializationOptions` if workspace-level active.                                          |
| `textDocument/didOpen`                                        | Run the Detector on file content + workspace root. Cache the `DetectionResult` for the URI. If Active: read on-disk `tsconfig.json` if present, call the Synthesiser, inject virtual type document notifications to the TS LSP. Then forward `didOpen`. |
| `textDocument/didClose`                                       | Forward. Evict the URI from the detection cache.                                                                                                                                                                                                        |
| `workspace/executeCommand` where command starts with `bundt/` | Handle internally (see debug protocol below). Do **not** forward to the TS LSP.                                                                                                                                                                         |
| All other messages                                            | Forward byte-for-byte.                                                                                                                                                                                                                                  |

### TS LSP → IDE

All messages are forwarded byte-for-byte without inspection or modification.

---

## Detection cache

Maps `DocumentUri → DetectionResult`. Maintained for the lifetime of the message loop.

- Populated on `textDocument/didOpen`.
- Evicted on `textDocument/didClose`.
- If `didOpen` arrives for a URI already in the cache (e.g. file reloaded), re-run detection and update the cache entry.

The cache is the authoritative source for whether injection has occurred for a given document. The Synthesiser is not called again for a document that is already cached as Active.

---

## Error handling

| Failure                                     | Behaviour                                                                                                                                                         |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TS LSP binary not found at startup          | Log a clear error to stderr and exit non-zero.                                                                                                                    |
| TS LSP subprocess exits unexpectedly        | Log the exit status, then exit `bundt` with the same exit code. The editor's built-in LSP restart behaviour takes over.                                           |
| Malformed JSON-RPC frame (either direction) | Log the raw bytes and the parse error. Skip the frame. Continue processing.                                                                                       |
| Synthesiser returns an error                | Log the error. Forward `textDocument/didOpen` without injection. The file gets standard TypeScript behaviour rather than Bun behaviour — degraded but not broken. |

The proxy must not crash due to data from either the IDE or the TS LSP. All recoverable errors are logged and skipped; only process-level failures (subprocess not found, subprocess exited) cause `bundt` to exit.

---

## Debug protocol (v0.3)

Handled by the Router in response to `workspace/executeCommand` requests with a `bundt/` command prefix. These are never forwarded to the TS LSP.

### `bundt/virtualConfig`

Params: `{ "uri": string }`

Returns the synthesised `tsconfig` value from the detection cache for the given URI. Returns a JSON-RPC error if the URI is not in the cache or its `DetectionResult` is `Inactive`.

### `bundt/detectionInfo`

Params: `{ "uri": string }`

Returns `{ "signal": string, "workspaceRoot": string | null }` from the detection cache. Returns a JSON-RPC error if the URI is not in the cache.

### Log level

When the `bundt.logLevel` setting is `"debug"` (communicated via `workspace/didChangeConfiguration`), the Router writes raw JSON-RPC frames to a log file. Log file path and format are specified in the v0.3 release design.

---

## Invariants

- The Router is the only component that performs I/O. The Detector and Synthesiser are pure functions called synchronously from the message loop.
- Frames in the TS LSP → IDE direction are never modified.
- A `bundt/` command is never forwarded to the TS LSP.
- The detection cache and the injection state are consistent: if a URI is cached as Active, the corresponding virtual type documents have already been sent to the TS LSP.
