# Detector

Decides whether a given file is in Bun context and, if so, which signal triggered it.

---

## Interface

**Inputs**

| Field            | Type            | Description                                             |
| ---------------- | --------------- | ------------------------------------------------------- |
| `file_content`   | `&str`          | Full text of the opened document                        |
| `workspace_root` | `Option<&Path>` | Workspace root path; absent for single-file invocations |

**Output**

```
DetectionResult
  Inactive
  Active { signal: Signal }

Signal
  Shebang
  BunImport
  Lockfile
```

The Detector has no output other than the `DetectionResult`. It does not produce config or type data.

---

## Signal evaluation

Signals are evaluated in priority order. Evaluation stops at the first match.

### 1. Shebang

The first line of `file_content` is exactly `#!/usr/bin/env bun`. No leading whitespace; no other arguments after `bun`.

### 2. BunImport

`file_content` contains a static ES module import or re-export from the `"bun"` specifier. Matching forms:

```
import ... from "bun"
import ... from 'bun'
import "bun"
import 'bun'
export ... from "bun"
export ... from 'bun'
```

The check is a fast string scan, not a full AST parse. It may produce false positives for occurrences inside comments or string literals — this is acceptable; a false positive activates Bun support on a file that doesn't need it, which is benign.

`require("bun")` is **not** a match. CommonJS require is not a Bun-specific signal.

### 3. Lockfile

`bun.lockb` exists as a file at `workspace_root`. If `workspace_root` is absent, this check is skipped — it is not an error.

---

## Invariants

- Returns exactly one `DetectionResult` per call.
- Does not write to disk or spawn subprocesses.
- The lockfile check is the only filesystem read; the shebang and import checks operate solely on `file_content`.
- Signal priority is fixed: Shebang > BunImport > Lockfile. The ordering cannot be changed at runtime.

---

## Error handling

- Lockfile stat fails for a reason other than "not found" (e.g. permissions): treat as absent and continue. Log at debug level.
- `file_content` is not valid UTF-8: the import scan returns no match. Shebang check returns no match. This is not an error.
