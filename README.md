# Solana Infra Doctor

[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/satyakwok/solana-infra-doctor/branch/main/graph/badge.svg)](https://codecov.io/gh/satyakwok/solana-infra-doctor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.76%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-blue.svg)](#commands)

**A local-first Rust CLI for Solana RPC production-readiness diagnostics, comparison, WebSocket checks, and redaction-safe reports.**

> Not just: *is this RPC online?*
> But: *which Solana RPC endpoint should I actually trust for this workload?*

Solana Infra Doctor diagnoses a Solana RPC endpoint, compares multiple
endpoints, checks WebSocket readiness, and produces terminal, JSON, and Markdown
reports — so you can decide which RPC to trust for bots, wallets, indexers, CI
pipelines, and infrastructure reviews.

- Diagnoses core JSON-RPC methods, blockhash freshness, slot data, latency, and
  performance samples.
- Compares two or more endpoints and scores each `0`–`100`.
- Tailors scoring to workload profiles: `general`, `wallet`, `bot`, `indexer`,
  `ci`.
- Checks WebSocket readiness (`slotSubscribe` time-to-first-event) with
  `sol-doctor ws`.
- Emits human-readable terminal output, JSON, and Markdown reports.

It is local-first, dependency-light, and built on raw HTTP JSON-RPC via
`reqwest`.

## CLI Preview

Compare two endpoints for a workload (`sol-doctor compare --profile bot`) — the
faster endpoint is not automatically the winner when it serves staler slots:

![sol-doctor compare summary table comparing two mainnet RPC endpoints for the bot profile](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/compare.png)

Diagnose a single endpoint (`sol-doctor check`):

![sol-doctor check readiness summary for a mainnet RPC endpoint](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/check.png)

Check WebSocket realtime readiness (`sol-doctor ws`):

![sol-doctor ws readiness summary showing connect, subscribe, and first notification steps](https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/docs/images/cli/ws.png)

Screenshots are real runs against public endpoints. Values vary by time, region,
and endpoint conditions. They are diagnostic snapshots, not provider guarantees.
The default view is concise; run with `--verbose` for full per-check detail. See
the [CLI Output Guide](docs/cli-output.md) and
[how these screenshots are made](docs/readme-screenshots.md).

## Install

```bash
cargo install solana-infra-doctor
```

Upgrade:

```bash
cargo install solana-infra-doctor --force
```

Verify:

```bash
sol-doctor --version
```

(Or build from source — see [Install From Source](#install-from-source).)

## Who Should Use This?

| User | Why it matters |
| --- | --- |
| Bot builders | Latency and slot freshness can affect execution quality. |
| Wallet/dApp backends | RPC reliability and blockhash readiness affect user-facing transactions. |
| Indexer operators | Slot freshness and data availability matter for indexing pipelines. |
| Infra teams | Compare providers before wiring endpoints into production systems. |
| CI pipelines | Use JSON output for deterministic readiness checks. |
| Consultants/auditors | Generate redaction-safe reports for RPC readiness reviews. |

## Commands

Check one RPC:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Compare multiple RPC endpoints:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot
```

Generate a Markdown report:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --report rpc-report.md
```

Check WebSocket readiness:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

JSON output (machine-readable, for CI):

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --json
```

Prefer to look before running? See [Example Outputs](#example-outputs) for
sample terminal output and Markdown reports.

## What It Checks

| Area | Checks |
| --- | --- |
| HTTP JSON-RPC | health, version, genesis hash, slot, blockhash, performance samples |
| Compare mode | score, latency, slot freshness, failed checks, best/worst endpoint |
| Network safety | rejects mixed-network comparisons by genesis hash |
| WebSocket | URL derivation, connect, `slotSubscribe`, first slot notification, unsubscribe, close |
| Output safety | redacts credentials and likely API keys in terminal, JSON, Markdown, and errors |

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

## Check Details (HTTP JSON-RPC)

`sol-doctor check` runs these JSON-RPC checks:

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
verdict using these thresholds:

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

Show full per-check detail (full redacted URL, per-method latencies, full hashes):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --verbose
```

Emit JSON (for automation — prefer this over parsing the human text):

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

Compare mode is intended for endpoints on the same Solana network. If endpoints
return different genesis hashes, Solana Infra Doctor rejects the comparison
because slot lag and ranking are not meaningful across networks.

Diagnose WebSocket readiness for realtime workloads:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

`ws` derives the WebSocket URL from the HTTP RPC URL (`https` → `wss`,
`http` → `ws`), connects, subscribes with `slotSubscribe`, measures the
time-to-first-slot-notification, unsubscribes, and closes. Override the derived
URL with `--ws wss://...` when a provider uses a separate WebSocket host, and
emit JSON with `--json`.

### Color output

Human output is colorized when stdout is a terminal. Color is **semantic**:
verdicts and `PASS`/`WARN`/`FAIL` markers carry status colors, labels are muted,
and section titles are emphasized. Control it with the global `--color` flag
(`check`, `compare`, and `ws` all accept it):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color auto    # default: color only on a TTY
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color always  # force color (e.g. piping into a pager that renders ANSI)
sol-doctor check --rpc https://api.mainnet-beta.solana.com --color never   # disable color
```

`--json` output is never colorized; the [`NO_COLOR`](https://no-color.org/)
environment variable and `TERM=dumb` are honored under `--color auto`. When color
is off, the output is byte-for-byte identical to the uncolored output. See the
[CLI Output Guide](docs/cli-output.md) for the full output reference.

## Human Output Example

A real run against `https://api.mainnet-beta.solana.com`. The default view is
concise (on a terminal it is colorized; see the [CLI Preview](#cli-preview)):

```text
Solana Infra Doctor · RPC Readiness

Target
Endpoint   api.mainnet-beta.solana.com

Result
GOOD      All RPC readiness checks passed
Latency   23 ms average
Checks    7 passed · 0 failed

Checks
Category       Status    Summary
Core           PASS      4 / 4
Blockhash      PASS      2 / 2
Performance    PASS      1 / 1

Tip: run with --verbose to see full details.
```

Run with `--verbose` for full per-check detail (full redacted URL, per-method
latencies, full hashes):

```text
Solana Infra Doctor · RPC Readiness

Target
RPC URL   https://api.mainnet-beta.solana.com/

Result
GOOD      All RPC readiness checks passed
Latency   22 ms average
Checks    7 passed · 0 failed

Checks

Core
- getHealth       PASS  86 ms  health is ok
- getVersion      PASS  13 ms  solana-core 4.0.0
- getGenesisHash  PASS  14 ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
- getSlot         PASS  11 ms  slot 424147058

Blockhash
- getLatestBlockhash  PASS  11 ms  4fzZUYN9uQR6HLTj5faRtJjbiXaLxUfz9k1T2N5ATELG
- isBlockhashValid    PASS  3 ms   latest blockhash is valid

Performance
- getRecentPerformanceSamples  PASS  19 ms  234389 transactions across 154 slots in 60s
```

## Compare Output Example

A real `bot`-profile comparison of two public mainnet endpoints. Note that the
lower-latency endpoint (#1) is not the winner: #2 serves fresher slots, which the
`bot` profile weighs more heavily. (Run with `--verbose` for full per-endpoint
detail.)

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC   Endpoint                      Verdict   Score    Latency   Slot lag
#1    api.mainnet-beta.solana.com   GOOD      83/100   20 ms     32 behind
#2    solana-rpc.publicnode.com     GOOD      90/100   98 ms     baseline

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.

Tip: run with --verbose to see full details per endpoint.
```

## JSON Output Example

```json
{
  "verdict": "GOOD",
  "rpc_url": "https://api.mainnet-beta.solana.com/",
  "summary": "All RPC readiness checks passed",
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
- [`examples/mixed-network-rejection.md`](examples/mixed-network-rejection.md)
  — how compare rejects endpoints from different Solana networks.
- [`examples/reports/compare-bot-live.md`](examples/reports/compare-bot-live.md)
  — a real `bot`-profile comparison of two public mainnet endpoints.

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

- `check` and `compare` use HTTP JSON-RPC; `sol-doctor ws` covers slot-subscription
  WebSocket readiness only (no account/log/program subscriptions yet).
- Compare checks currently run sequentially.
- Scores are deterministic heuristics, not provider guarantees.
- This is a local-first CLI, not a hosted monitoring service.
- No Token Program, Token-2022, transaction simulation, or account indexing
  checks yet.
- No Solana SDK dependencies are used yet.
- No Prometheus exporter, dashboard, hosted cloud service, marketplace, token,
  NFT, points, airdrop, or governance features.

## Security and Privacy

Solana Infra Doctor redacts credentials and likely API keys from displayed RPC
URLs, error messages, JSON output, and Markdown reports. Avoid sharing raw
private RPC URLs.

## Practical Use Cases

Solana Infra Doctor can produce redaction-safe diagnostic artifacts for:

- RPC provider comparison before choosing an endpoint
- bot/indexer readiness reviews
- wallet/backend RPC checks
- CI readiness checks (JSON output, exit codes)
- short technical RPC audit reports

For a worked example of turning the CLI output into a shareable report, see
[`docs/rpc-readiness-report.md`](docs/rpc-readiness-report.md).

This repository does not provide hosted monitoring, paid SaaS, or SLA
guarantees. It is a local diagnostic tool.

## What This Is Not

- Not a hosted monitoring service
- Not an SLA provider
- Not a replacement for provider observability
- Not a trading performance guarantee
- Not a dashboard or SaaS product
- Not affiliated with or endorsed by Solana Foundation

## Coverage Policy

CI enforces at least `95%` line coverage with `cargo llvm-cov`. Coverage reports
are generated as `lcov.info`, uploaded to Codecov, and ignored locally so the
report artifact is not committed.

## Roadmap

Grounded in usefulness, not feature count.

**Near-term**

- Repeat sampling mode for better p50/p95 latency and short-window error-rate
  signals.
- Richer report templates.
- A GitHub Action wrapper for CI.
- More example reports.

**Later**

- Optional local benchmark history file.
- A provider comparison playbook.
- Install/distribution improvements.

**Not now**

- Hosted dashboard, SaaS, user accounts, database, alerting, or a paid API.

## License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

Copyright 2026 Satya Kwok.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated
with or endorsed by Solana Foundation.
