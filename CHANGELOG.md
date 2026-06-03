# Changelog

## 0.2.1 - 2026-06-03

- Deduplicate thiserror dependency versions by updating the direct dependency to thiserror 2.

## 0.2.0 - 2026-06-03

- Add `sol-doctor ws` for WebSocket readiness diagnostics: derive the
  WebSocket URL from the HTTP RPC URL (or override with `--ws`), connect,
  `slotSubscribe`, measure time-to-first-slot-notification, unsubscribe, and
  close, with a GOOD/WARNING/BAD/UNKNOWN verdict and JSON output.
- Redact credentials and API keys in the derived WebSocket URL.
- Align the CLI help text with the current production-readiness positioning.

## 0.1.4 - 2026-06-03

- Align CLI help text with the current production-readiness positioning.
- Add an example for mixed-network comparison rejection.
- Clean up stale merged branch metadata.

## 0.1.3 - 2026-06-03

- Harden RPC URL and API-key redaction across terminal, JSON, Markdown, and
  error output.
- Prevent query-string API keys from leaking through HTTP client error
  messages.
- Redact likely provider tokens in URL paths (for example `/v2/<token>`) and
  mask basic-auth credentials.

## 0.1.2 - 2026-06-03

- Compare mode now rejects mixed-network endpoints with mismatched genesis
  hashes; slot lag and best/worst ranking are disabled across different
  Solana networks.
- Recommendations now describe the latency-versus-slot-freshness tradeoff more
  accurately instead of mislabeling a faster-but-staler endpoint as a
  latency risk.

## 0.1.1 - 2026-06-03

- Add example diagnostic outputs under `examples/` (terminal output and Markdown reports).
- Add crates.io publishing metadata: `readme`, `keywords`, and `categories`.
- Refine the package description.

## 0.1.0 - 2026-06-03

- Add initial Rust CLI foundation with `sol-doctor check --rpc <RPC_URL>`.
- Validate RPC URLs and support a per-request timeout.
- Run raw HTTP JSON-RPC checks for `getHealth`, `getVersion`, `getGenesisHash`,
  `getSlot`, `getLatestBlockhash`, `isBlockhashValid`, and
  `getRecentPerformanceSamples`.
- Group checks into Core, Blockhash, and Performance categories with error
  classification.
- Measure per-method and average latency.
- Add multi-RPC `compare` mode with workload profiles (general, wallet, bot,
  indexer, ci).
- Add deterministic scoring, verdicts, and exit codes.
- Add human-readable, JSON, and Markdown report output, with URL redaction.
- Add GitHub Actions CI, a 95% line-coverage gate, Codecov configuration, and
  tests.
