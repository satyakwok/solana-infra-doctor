# Solana Infra Doctor

[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-v0.1--foundation-lightgrey.svg)](#current-limitations)

A lightweight Rust CLI for diagnosing Solana RPC and infrastructure health.

Solana Infra Doctor checks whether a Solana RPC endpoint is online and usable
for builders, bots, wallets, indexers, and production applications. It is
intentionally small, dependency-light, and built on raw HTTP JSON-RPC via
`reqwest`.

## Why This Exists

A Solana RPC endpoint can accept connections while still being unsuitable for
production workloads. Basic uptime checks do not tell you whether core RPC
calls work, recent blockhashes are usable, or the endpoint can return recent
performance data.

Solana Infra Doctor gives developers a fast local diagnostic that can be used
before wiring an endpoint into application code, CI jobs, infrastructure
automation, or operational runbooks.

## What It Checks Today

`sol-doctor check` currently runs these JSON-RPC checks:

| Category | Method | Purpose |
| --- | --- | --- |
| Core | `getHealth` | Confirms the node reports healthy status. |
| Core | `getVersion` | Confirms validator software version metadata is available. |
| Core | `getGenesisHash` | Confirms the endpoint can identify its cluster genesis hash. |
| Core | `getSlot` | Confirms the endpoint can return current slot data. |
| Blockhash | `getLatestBlockhash` | Confirms the endpoint can produce a recent blockhash. |
| Blockhash | `isBlockhashValid` | Confirms the latest returned blockhash is valid. |
| Performance | `getRecentPerformanceSamples` | Confirms recent performance sample data is available. |

The CLI measures latency for each method and calculates an average latency
verdict using these v0.1 thresholds:

- `GOOD`: average latency is less than or equal to 500ms.
- `WARNING`: average latency is greater than 500ms and less than or equal to
  1500ms.
- `BAD`: average latency is greater than 1500ms or repeated timeouts occur.

Error kinds are classified as:

- `invalid_url`
- `timeout`
- `http_error`
- `rpc_error`
- `malformed_response`
- `unknown_error`

## Install From Source

```bash
git clone https://github.com/satyakwok/solana-infra-doctor.git
cd solana-infra-doctor
cargo build --release
```

The binary is built at:

```bash
./target/release/sol-doctor
```

## Usage

Check an RPC endpoint:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Emit JSON:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --json
```

Use a custom per-request timeout:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --timeout-ms 3000
```

Make warning behavior explicit for CI:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --fail-on-warning
```

`--fail-on-warning` does not change the exit code mapping. `WARNING` still
exits with code `1`; the output makes the CI policy explicit.

## Human Output Example

```text
Solana Infra Doctor
===================
RPC URL: https://api.mainnet-beta.solana.com/
Verdict: GOOD
Summary: all RPC readiness checks succeeded
Average latency: 42ms

Checks:

Core:
- getHealth                    OK       35ms  health is ok
- getVersion                   OK       39ms  solana-core 4.0.0
- getGenesisHash               OK       41ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
- getSlot                      OK       38ms  slot 424013263

Blockhash:
- getLatestBlockhash           OK       44ms  7xKXtgQv...example
- isBlockhashValid             OK       40ms  latest blockhash is valid

Performance:
- getRecentPerformanceSamples  OK       47ms  124000 transactions across 64 slots in 60s
```

## JSON Output Example

```json
{
  "verdict": "GOOD",
  "rpc_url": "https://api.mainnet-beta.solana.com/",
  "summary": "all RPC readiness checks succeeded",
  "average_latency_ms": 42,
  "fail_on_warning": false,
  "checks": [
    {
      "category": "core",
      "method": "getHealth",
      "status": "success",
      "latency_ms": 35,
      "detail": "health is ok",
      "error_kind": null,
      "critical": true
    },
    {
      "category": "blockhash",
      "method": "isBlockhashValid",
      "status": "success",
      "latency_ms": 40,
      "detail": "latest blockhash is valid",
      "error_kind": null,
      "critical": true
    },
    {
      "category": "performance",
      "method": "getRecentPerformanceSamples",
      "status": "success",
      "latency_ms": 47,
      "detail": "124000 transactions across 64 slots in 60s",
      "error_kind": null,
      "critical": false
    }
  ]
}
```

## Exit Codes

| Code | Verdict | Meaning |
| --- | --- | --- |
| `0` | `GOOD` | Required checks passed and latency is acceptable. |
| `1` | `WARNING` | Endpoint is reachable, but latency is elevated or one non-critical check failed. |
| `2` | `BAD` | URL is invalid, endpoint is unreachable, critical checks failed, repeated timeouts occurred, or latency is too high. |
| `3` | `UNKNOWN` or internal error | Not enough data for a reliable verdict, or an unexpected internal error occurred. |

## Current Limitations

- HTTP JSON-RPC only; WebSocket diagnostics are not implemented yet.
- Checks run sequentially in v0.1.
- Verdicts are deterministic but intentionally conservative.
- No Solana SDK dependencies are used yet.
- No Token Program, Token-2022, transaction simulation, account indexing, or
  endpoint comparison checks yet.
- No Prometheus exporter, dashboard, hosted cloud service, marketplace, token,
  NFT, points, airdrop, or governance features.

## Roadmap

- Add optional cluster expectation checks.
- Add endpoint comparison mode.
- Add richer rate-limit and provider-degradation diagnostics.
- Add transaction simulation readiness checks without pulling heavy SDK
  dependencies too early.
- Add machine-readable output refinements for CI and infrastructure automation.
- Consider parallel check execution once the v0.1 behavior is stable.

## License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

Copyright 2026 Satya Kwok.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated
with or endorsed by Solana Foundation.
