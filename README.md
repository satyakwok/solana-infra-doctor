# Solana Infra Doctor

[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/satyakwok/solana-infra-doctor/branch/main/graph/badge.svg)](https://codecov.io/gh/satyakwok/solana-infra-doctor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-v0.1.1-blue.svg)](#current-limitations)

**A Rust CLI for Solana RPC production-readiness diagnostics, comparison, and reports.**

> Not just: *is this RPC online?*
> But: *which RPC should I actually trust for this workload?*

Solana Infra Doctor diagnoses a Solana RPC endpoint, compares multiple
endpoints, and produces terminal, JSON, and Markdown reports — so you can decide
which RPC to trust for wallets, bots, indexers, CI, and production applications.

- Diagnoses core JSON-RPC methods, blockhash freshness, slot data, latency, and
  performance samples.
- Compares two or more endpoints and scores each `0`–`100`.
- Tailors scoring to workload profiles: `general`, `wallet`, `bot`, `indexer`,
  `ci`.
- Emits human-readable terminal output, JSON, and Markdown reports.

It is local-first, dependency-light, and built on raw HTTP JSON-RPC via
`reqwest`.

## Quick Start

Diagnose a single endpoint:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Compare endpoints for a workload and write a report:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://api.devnet.solana.com \
  --profile bot \
  --report rpc-report.md
```

Prefer to look before running? See [Example Outputs](#example-outputs) for
sample terminal output and Markdown reports.

## Workload Profiles

| Profile | Use case | Optimized for |
|---|---|---|
| `general` | Default diagnostics | balanced checks |
| `wallet` | Wallets and dApps | reliability and blockhash readiness |
| `bot` | Bots and automation | latency and slot freshness |
| `indexer` | Indexers/data pipelines | slot lag and data availability |
| `ci` | CI/deployment checks | deterministic pass/fail behavior |

## Why This Exists

A Solana RPC endpoint can be reachable and still be unsuitable for real
workloads. A basic uptime check does not tell you whether:

- core JSON-RPC methods actually work
- recent blockhashes are usable
- slot data is fresh
- latency is acceptable
- performance samples are available
- one endpoint is better than another for a specific workload

Solana Infra Doctor answers those questions with a fast local diagnostic you can
run before wiring an endpoint into application code, CI jobs, infrastructure
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

Compare two or more RPC endpoints:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com
```

Compare endpoints for a specific workload profile:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --profile bot
```

Emit compare results as JSON:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --json
```

Write a Markdown comparison report:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://example-rpc-provider.com \
  --profile indexer \
  --report rpc-report.md
```

Compare mode supports these profiles:

| Profile | Use case |
| --- | --- |
| `general` | Balanced default scoring for general production readiness. |
| `wallet` | Emphasizes core RPC success and latest blockhash validity. |
| `bot` | Penalizes elevated latency and high slot lag for latency-sensitive workloads. |
| `indexer` | Penalizes slot lag and unavailable recent performance samples. |
| `ci` | Uses strict recommendation text for pass-gate decisions. |

Compare mode helps choose RPC endpoints for wallet, bot, indexer, and CI
workloads by scoring each endpoint from `0` to `100`, calculating slot lag
against the freshest observed endpoint, listing failed checks, and recommending
the best and worst endpoint.

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

## Compare Output Example

```text
Solana Infra Doctor — RPC Compare

Profile: bot

RPC #1
URL: https://api.mainnet-beta.solana.com/
Verdict: GOOD
Score: 90/100
Slot: 347000000
Slot lag: baseline
Average latency: 142ms
Failed checks: none

RPC #2
URL: https://***.provider.com/
Verdict: WARNING
Score: 15/100
Slot: 346999700
Slot lag: 300 slots behind
Average latency: 812ms
Failed checks: getRecentPerformanceSamples
Notes:
- Average latency is high for latency-sensitive bot workloads.
- Slot lag is high for slot-sensitive bot workloads.

Recommendation:
Best RPC: RPC #1
Worst RPC: RPC #2
RPC #1 is recommended for bot workloads.
Avoid RPC #2 for latency-sensitive or slot-sensitive workloads.
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

## Compare JSON Output Example

```json
{
  "profile": "bot",
  "endpoints": [
    {
      "index": 1,
      "url": "https://api.mainnet-beta.solana.com/",
      "verdict": "GOOD",
      "score": 90,
      "slot": 347000000,
      "slot_lag": 0,
      "average_latency_ms": 142,
      "failed_checks": [],
      "blockhash_valid": true,
      "notes": []
    },
    {
      "index": 2,
      "url": "https://***.provider.com/",
      "verdict": "WARNING",
      "score": 15,
      "slot": 346999700,
      "slot_lag": 300,
      "average_latency_ms": 812,
      "failed_checks": ["getRecentPerformanceSamples"],
      "blockhash_valid": true,
      "notes": [
        "Average latency is high for latency-sensitive bot workloads.",
        "Slot lag is high for slot-sensitive bot workloads."
      ]
    }
  ],
  "best_endpoint_index": 1,
  "worst_endpoint_index": 2,
  "recommendation": "Best RPC: RPC #1\nWorst RPC: RPC #2\nRPC #1 is recommended for bot workloads.\nAvoid RPC #2 for latency-sensitive or slot-sensitive workloads."
}
```

## Markdown Report Example

```markdown
# Solana Infra Doctor RPC Compare Report

Profile: `indexer`

## Summary

- Best RPC: RPC #1
- Worst RPC: RPC #2

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 90/100 | 347000000 | baseline | 142ms | none | yes |
```

## Example Outputs

Sample outputs are committed under [`examples/`](examples/) so you can inspect
what the tool produces without running it first. They are illustrative
diagnostic runs, not provider benchmarks.

- [`examples/terminal/check-mainnet.txt`](examples/terminal/check-mainnet.txt)
  — single-RPC `check` terminal output.
- [`examples/terminal/compare-bot.txt`](examples/terminal/compare-bot.txt)
  — `compare` terminal output for the `bot` profile.
- [`examples/reports/compare-bot-report.md`](examples/reports/compare-bot-report.md)
  — Markdown comparison report for the `bot` profile.
- [`examples/reports/compare-indexer-report.md`](examples/reports/compare-indexer-report.md)
  — Markdown comparison report for the `indexer` profile.

These reports are useful as a readiness signal for RPC comparison, bot/indexer
readiness review, CI discussion, and consulting-style diagnostics. Scores are
deterministic heuristics, not a guarantee of provider behavior.

## Exit Codes

| Code | Verdict | Meaning |
| --- | --- | --- |
| `0` | `GOOD` | Required checks passed and latency is acceptable. |
| `1` | `WARNING` | Endpoint is reachable, but latency is elevated or one non-critical check failed. |
| `2` | `BAD` | URL is invalid, endpoint is unreachable, critical checks failed, repeated timeouts occurred, or latency is too high. |
| `3` | `UNKNOWN` or internal error | Not enough data for a reliable verdict, or an unexpected internal error occurred. |

## Current Limitations

- HTTP JSON-RPC only; WebSocket diagnostics are not included yet.
- Compare checks currently run sequentially.
- Scores are deterministic heuristics, not provider guarantees.
- This is a local-first CLI, not a hosted monitoring service.
- No Token Program, Token-2022, transaction simulation, or account indexing
  checks yet.
- No Solana SDK dependencies are used yet.
- No Prometheus exporter, dashboard, hosted cloud service, marketplace, token,
  NFT, points, airdrop, or governance features.

## Coverage Policy

CI enforces at least `95%` line coverage with `cargo llvm-cov`. Coverage reports
are generated as `lcov.info`, uploaded to Codecov, and ignored locally so the
report artifact is not committed.

## Roadmap

- Add optional cluster expectation checks.
- Refine endpoint comparison scoring as more real-world diagnostics are added.
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
