# Yellowstone gRPC — real credentialed mainnet run

This is **real credentialed mainnet Yellowstone gRPC evidence**, not a mock.

- **Documented (UTC):** 2026-06-05T20:55:39Z
- **Tool version:** sol-doctor 0.13.0
- **Endpoint label:** Tatum mainnet Yellowstone gRPC
- **Endpoint (safe):** `https://solana-mainnet-grpc.gateway.tatum.io`
- **Token handling:** the `x-token` was passed through the `YELLOWSTONE_X_TOKEN`
  environment variable using `--x-token-env`. The token is **never** accepted on
  the command line, printed, serialized, or committed. No token appears in this
  repository (verified).

## Exact command

```bash
export YELLOWSTONE_X_TOKEN="<your Tatum API key>"   # never committed
sol-doctor grpc check \
  --grpc https://solana-mainnet-grpc.gateway.tatum.io \
  --x-token-env YELLOWSTONE_X_TOKEN \
  --rpc https://api.mainnet-beta.solana.com \
  --verbose
```

## Evidence files

- `examples/terminal/grpc-check-real-mainnet-tatum-redacted.txt`
- `examples/reports/grpc-check-real-mainnet-tatum-redacted.md`
- `examples/terminal/grpc-check-real-mainnet-tatum-redacted-retry.txt` (retry, longer timeout/duration)
- `examples/reports/grpc-check-real-mainnet-tatum-redacted-retry.md`

A **second run** (`--timeout-ms 30000 --duration 20000`) reproduced the same
**degraded** verdict against live mainnet (fresh slot, geyser 13.1.0): transport,
auth, ping, version, and `GetSlot` passed; the slot stream still did not become
ready; `GetBlockHeight` / `GetLatestBlockhash` were still rate-limited (429). A
longer stream window did not change the outcome — this is a stable real-provider
condition, not a one-off. Token passed via `YELLOWSTONE_X_TOKEN`; not committed.

## Result summary — DEGRADED / BAD

The endpoint was reachable and accepted authentication. Transport, ping, version,
and slot checks passed against live mainnet. The run was **not fully healthy**
because the slot stream did not produce a slot update before closing, and two
unary methods were rate-limited with HTTP 429.

| Check | Status | Detail |
| --- | --- | --- |
| Transport | PASS | Connected over TLS (HTTP/2) |
| Authentication | PASS | Token accepted |
| Ping | PASS | pong |
| GetVersion | PASS | yellowstone-grpc-geyser 13.0.0+solana.4.0.0 |
| GetSlot | PASS | real mainnet slot (e.g. 424535648) |
| GetBlockHeight | FAIL | HTTP 429 Too Many Requests |
| GetLatestBlockhash | FAIL | HTTP 429 Too Many Requests |
| IsBlockhashValid | SKIP | no blockhash (GetLatestBlockhash failed) |
| Stream (slot-only Subscribe) | FAIL | stream closed before a slot update |
| Cross-check (gRPC vs HTTP slot) | SKIP | no gRPC slot observed to compare |
| Token leak | PASS | no token found in examples/docs/README |

## Why the verdict is degraded / BAD

The overall verdict is driven by the slot stream and the unary checks. Here the
slot **stream closed before producing a slot update**, and `GetBlockHeight` /
`GetLatestBlockhash` were **rate-limited (429)** by the provider. Authentication
and a live `GetSlot` succeeded, so this is not a transport or auth failure — it is
a real, **degraded provider readiness** result.

## What this evidence proves (and does not)

- ✅ Proves that `sol-doctor grpc check` can test a **real credentialed mainnet
  gRPC endpoint** and **detect degraded readiness** (rate-limiting, stream not
  ready) accurately, with no token leak.
- ❌ Does **not** prove the endpoint is fully healthy, and does **not** mean all
  gRPC checks passed.

## Classification

**PASS_REAL_RUN_DEGRADED** — real run against a credentialed mainnet endpoint;
verdict degraded/BAD.

`grpc compare` is **not** covered by this run: it ranks two or more endpoints and
requires multiple credentialed endpoints/tokens, which were not available. It
remains BLOCKED_PRIVATE_CREDENTIALS_REQUIRED.
