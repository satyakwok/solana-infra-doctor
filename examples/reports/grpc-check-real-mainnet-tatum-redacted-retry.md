# Solana Infra Doctor — Yellowstone gRPC Readiness Report

- Endpoint: `https://solana-mainnet-grpc.gateway.tatum.io/`
- HTTP RPC: `https://api.mainnet-beta.solana.com/`
- Verdict: `BAD`
- Summary: gRPC endpoint is reachable but the slot stream is not ready
- Token provided: yes
- Connect latency: 13 ms

## Checks

| Category | Status | Summary |
| --- | --- | --- |
| Transport | `PASS` | Connected over TLS (HTTP/2) |
| Authentication | `PASS` | Token accepted |
| Unary | `WARN` | 0 / 5 supported checks passed |
| Stream | `FAIL` | stream closed before a slot update |
| Cross-check | `SKIP` | no gRPC slot observed to compare |

## Unary methods

| Method | Status | Latency | Detail |
| --- | --- | --- | --- |
| Ping | `FAIL` | 6 ms | protocol error: received message with invalid compression flag: 123 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests |
| GetVersion | `FAIL` | 6 ms | protocol error: received message with invalid compression flag: 123 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests |
| GetSlot | `FAIL` | 5 ms | protocol error: received message with invalid compression flag: 123 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests |
| GetBlockHeight | `FAIL` | 6 ms | protocol error: received message with invalid compression flag: 123 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests |
| GetLatestBlockhash | `FAIL` | 5 ms | protocol error: received message with invalid compression flag: 123 (valid flags are 0 and 1) while receiving response with status: 429 Too Many Requests |
| IsBlockhashValid | `SKIP` | — | skipped (no blockhash from GetLatestBlockhash) |

## Slot stream

- Status: `FAIL`
- Updates observed: 0

## HTTP RPC cross-check

- Cross-check could not be completed.

## Warnings

- 5 unary method check(s) failed; the endpoint may have limited unary support

## Remediation

- the slot stream did not become ready; compare against the HTTP RPC slot with --rpc and check server logs

## Disclaimer

This report is a point-in-time diagnostic snapshot. It is not an SLA, security audit, or guarantee of future endpoint performance.

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
