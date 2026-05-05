# Changelog

All notable changes to this project will be documented in this file.

## [v0.1.0] - 2026-05-04

### Added

- Transparent JSON-RPC proxy: `bundt <lsp-binary> [args…]` sits between the IDE and a downstream TypeScript LSP, forwarding all LSP traffic unchanged (US-12).
- LSP Content-Length frame codec (`framing` module): reads and writes the standard `Content-Length: N\r\n\r\n` envelope used by all LSP implementations.
- Bidirectional forwarding loop: two concurrent tasks relay IDE → LSP and LSP → IDE independently, so neither direction can stall the other.
- Graceful shutdown: when either side closes its connection the proxy drains in-flight frames and exits with the same code as the downstream LSP (US-27).
- Malformed JSON-RPC frames are logged and skipped rather than crashing the proxy (US-28).
- Clear error message when the downstream LSP binary is not found (US-26).
- Platform binaries: Linux x86_64, macOS x86_64, macOS aarch64, Windows x86_64.
