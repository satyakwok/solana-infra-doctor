<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/assets/logo-wordmark-dark.svg" />
    <img alt="Solana Infra Doctor" src="https://raw.githubusercontent.com/satyakwok/solana-infra-doctor/main/assets/logo-wordmark-light.svg" width="640" />
  </picture>
</p>

<p align="center">
  <b>English</b> · <a href="./README.id.md">Bahasa Indonesia</a>
</p>

# Solana Infra Doctor

[![crates.io](https://img.shields.io/crates/v/solana-infra-doctor.svg)](https://crates.io/crates/solana-infra-doctor)
[![GitHub Marketplace](https://img.shields.io/badge/Marketplace-Solana%20Infra%20Doctor-blue?logo=github)](https://github.com/marketplace/actions/solana-infra-doctor)
[![CI](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml/badge.svg)](https://github.com/satyakwok/solana-infra-doctor/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/satyakwok/solana-infra-doctor/branch/main/graph/badge.svg)](https://codecov.io/gh/satyakwok/solana-infra-doctor)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-blue.svg)](#commands)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/satyakwok/solana-infra-doctor)

**A local-first Rust CLI for Solana RPC production-readiness diagnostics, comparison, WebSocket checks, and redaction-safe reports.**

> Not just: *is this RPC online?*
> But: *which Solana RPC endpoint should I actually trust for this workload?*

Solana Infra Doctor diagnoses a Solana RPC endpoint, compares multiple
endpoints, checks WebSocket readiness, and produces terminal, JSON, and Markdown
reports — so you can decide which RPC to trust for bots, wallets, indexers, CI
pipelines, and infrastructure reviews.

- Diagnoses core JSON-RPC methods, blockhash freshness, slot data, latency, and
  performance samples.
- Checks SPL Token Program and Token-2022 readiness (whether the RPC serves the
  token programs as executable accounts) via `getAccountInfo`.
- Compares two or more endpoints and scores each `0`–`100`.
- Tailors scoring to workload profiles: `general`, `wallet`, `bot`, `indexer`,
  `ci`.
- Checks WebSocket readiness (`slotSubscribe` time-to-first-event) with
  `sol-doctor ws`.
- Checks **Yellowstone gRPC** readiness (connect, optional `x-token` auth, safe
  unary probes, and a slot-only stream) with `sol-doctor grpc check`.
- Compares **Yellowstone gRPC** endpoints by connect latency, time-to-first-event,
  and slot freshness for a workload profile with `sol-doctor grpc compare`.
- Emits human-readable terminal output, JSON, and Markdown reports.

It is local-first and dependency-light: HTTP JSON-RPC via `reqwest`, WebSocket
via `tokio-tungstenite`, and Yellowstone gRPC via `tonic` with the official
`yellowstone-grpc-proto` definitions (no full Solana/Agave SDK).

## CLI Preview

Compare two endpoints for a workload (`sol-doctor compare --profile bot`) — the
faster endpoint is not automatically the winner when it serves staler slots:

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC   Endpoint                      Verdict   Score     Latency   Slot lag
#1    api.mainnet-beta.solana.com   GOOD      99/100    16 ms     32 behind
#2    solana-rpc.publicnode.com     GOOD      100/100   105 ms    baseline

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.
```

Diagnose a single endpoint (`sol-doctor check`):

```text
Solana Infra Doctor · RPC Readiness

Target
Endpoint   api.mainnet-beta.solana.com

Result
GOOD         All RPC readiness checks passed
Latency      10 ms average
Checks       11 passed · 0 failed
Block time   13s behind (finalized)
Fee market   median 0 micro-lamports/CU
Token        Token Program ready · Token-2022 ready

Checks
Category       Status    Summary
Core           PASS      4 / 4
Blockhash      PASS      2 / 2
Performance    PASS      3 / 3
Token          PASS      2 / 2
```

Check WebSocket realtime readiness (`sol-doctor ws`):

```text
Solana Infra Doctor · WebSocket Readiness

Target
RPC         api.mainnet-beta.solana.com
WebSocket   wss://api.mainnet-beta.solana.com/

Result
GOOD   WebSocket readiness checks passed

Checks
Check                 Status    Detail
Connect               PASS      94 ms
Subscribe             PASS      slotSubscribe · id 1
First notification    PASS      132 ms · slot 424282423
Unsubscribe           PASS
Close                 PASS
```

These are real runs against public endpoints. Values vary by time, region, and
endpoint conditions; they are diagnostic snapshots, not provider guarantees. The
concise default is shown here; run with `--verbose` for full per-check detail
(see the verbose examples below). See the [CLI Output Guide](docs/cli-output.md).

## Install

### Prebuilt binary (no Rust toolchain)

Each release attaches prebuilt binaries for Linux (gnu + static musl), macOS
(Intel + Apple Silicon), and Windows. With
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall solana-infra-doctor
```

Or download the archive for your platform from the
[latest release](https://github.com/satyakwok/solana-infra-doctor/releases/latest)
(named `sol-doctor-<target>`), extract it, and put `sol-doctor` on your `PATH`.

### From crates.io (compiles from source)

```bash
cargo install solana-infra-doctor
```

Upgrade with `cargo install solana-infra-doctor --force` (or
`cargo binstall --force solana-infra-doctor`).

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

Check Yellowstone gRPC readiness:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint
```

Compare multiple Yellowstone gRPC endpoints:

```bash
sol-doctor grpc compare \
  --grpc https://example-yellowstone-a \
  --grpc https://example-yellowstone-b \
  --profile latency
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
| Freshness & fees | block-time freshness (`getBlockTime`), recent prioritization fees |
| Compare mode | score, latency, slot freshness, block-time freshness, failed checks, best/worst endpoint |
| Network safety | rejects mixed-network comparisons by genesis hash |
| WebSocket | URL derivation, connect, `slotSubscribe`/`logsSubscribe`, first notification, reconnect, unsubscribe, close |
| Yellowstone gRPC | connect + TLS/HTTP-2, optional `x-token` auth, unary probes (`Ping`/`GetVersion`/`GetSlot`/`GetBlockHeight`/`GetLatestBlockhash`/`IsBlockhashValid`), slot-only stream first-event, optional HTTP RPC slot cross-check |
| Yellowstone gRPC compare | rank endpoints by verdict, connect latency, time-to-first-event, slot freshness, and stream stability for `general`/`latency`/`indexer` profiles, with per-endpoint `x-token` env pairing |
| Output safety | redacts credentials and likely API keys (and never prints the gRPC `x-token`) in terminal, JSON, Markdown, and errors |

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
| Performance | `getBlockTime` | Measures how far the latest finalized block time lags wall clock (a freshness signal used in scoring). |
| Performance | `getRecentPrioritizationFees` | Surfaces the median recent priority fee as fee-market context (chain-wide, not a per-endpoint score signal). |
| Token | `getAccountInfo` | Confirms the SPL Token Program account is served as an executable program. |
| Token | `getAccountInfo` | Confirms the Token-2022 (Token Extensions) program account is served as an executable program. |

`Token` checks confirm the endpoint serves the canonical token programs
(`Tokenkeg…` and `TokenzQd…`) as executable accounts — the readiness most
token-touching workloads (wallets, trading bots, token indexers) depend on. They
are informational: a failure caps the verdict at `WARNING` rather than `BAD`, and
profile scoring rewards token readiness for the `wallet`, `bot`, and `indexer`
profiles. See [`examples/reports/token-readiness-report.md`](examples/reports/token-readiness-report.md)
for a real comparison.

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

Probe latency multiple times and report percentiles (a single sample hides tail
latency):

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --samples 20
```

This adds a `Samples` line (`p50 … · p95 …`) to human output and a
`latency_samples` object to JSON. The default is a single sample, and `--samples`
does not change the verdict.

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
the best and worst endpoint. Endpoints are checked **concurrently**, so the run
takes about as long as the slowest endpoint rather than the sum of all of them.

The HTTP client is also resilient: each endpoint is rate-limited (to stay polite
toward public RPCs) and transient failures (timeouts, connection errors, HTTP
429) are retried with exponential backoff. None of this changes the CLI, the
verdict, or the output shape.

Compare mode is intended for endpoints on the same Solana network. If endpoints
return different genesis hashes, Solana Infra Doctor rejects the comparison
because slot lag and ranking are not meaningful across networks.

Diagnose WebSocket readiness for realtime workloads:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

`ws` derives the WebSocket URL from the HTTP RPC URL (`https` → `wss`,
`http` → `ws`), connects, subscribes with `slotSubscribe`, measures the
time-to-first-notification, unsubscribes, and closes. Override the derived URL
with `--ws wss://...` when a provider uses a separate WebSocket host, and emit
JSON with `--json`.

If a connection fails to establish or drops before the first notification, `ws`
**reconnects with exponential backoff** (up to a few attempts) before giving up;
the number of reconnects is reported. Choose a different subscription to test
with `--subscription` (`slot`, the default, or `logs`):

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com --subscription logs
```

### Yellowstone gRPC readiness

Check whether a Yellowstone gRPC endpoint is reachable, authenticated,
responsive, and streaming fresh slot data:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint
```

Most Yellowstone endpoints require an `x-token`. Provide it via an **environment
variable** — the token is never accepted directly on the command line and is
never printed, serialized, or logged:

```bash
export YELLOWSTONE_X_TOKEN="your-token"

sol-doctor grpc check \
  --grpc https://example-yellowstone-endpoint \
  --x-token-env YELLOWSTONE_X_TOKEN
```

Optionally cross-check the gRPC stream's latest slot against an HTTP RPC endpoint
(reusing the same redaction-safe RPC client):

```bash
sol-doctor grpc check \
  --grpc https://example-yellowstone-endpoint \
  --x-token-env YELLOWSTONE_X_TOKEN \
  --rpc https://api.mainnet-beta.solana.com
```

`grpc check` validates and redacts the gRPC URL, connects (TLS + HTTP/2 for
`https`), attaches the `x-token` only when supplied, runs safe **unary** probes
(`Ping`, `GetVersion`, `GetSlot`, `GetBlockHeight`, `GetLatestBlockhash`,
`IsBlockhashValid`), and opens a **narrow slot-only** `Subscribe` stream to
measure time-to-first-slot-update and the latest observed slot. It is safe by
default: it never sends transactions, never modifies remote state, never
subscribes to accounts/transactions/blocks, and bounds every connection,
request, and stream with a deadline.

A method that returns `UNIMPLEMENTED` is treated as an optional capability
(`SKIP`), not a failure, because some Yellowstone deployments expose only the
`Subscribe` stream. The verdict is driven by transport, authentication, and the
slot stream; a degraded unary check or a large slot gap is a `WARNING`, not
`BAD`.

Options:

| Flag | Purpose |
| --- | --- |
| `--grpc <URL>` | Yellowstone gRPC endpoint (`http`/`https`). Required. |
| `--x-token-env <ENV>` | Read the `x-token` from this environment variable. |
| `--rpc <URL>` | Optional HTTP RPC endpoint for a slot-freshness cross-check. |
| `--timeout-ms <MS>` | Connection and per-request timeout (default `10000`). |
| `--duration <MS>` | Bounded slot-stream observation window (default `5000`). |
| `--json` | Machine-readable JSON (includes `schema_version`). |
| `--report <PATH>` | Write a Markdown report. |
| `--verbose` | Show per-method detail, the cross-check, and remediation hints. |

Emit JSON or write a Markdown report:

```bash
sol-doctor grpc check --grpc https://example-yellowstone-endpoint --json
sol-doctor grpc check --grpc https://example-yellowstone-endpoint --report yellowstone-grpc-report.md
```

Example human output (structure shown; values vary by endpoint and moment):

```text
Solana Infra Doctor · Yellowstone gRPC Readiness

Target
Endpoint     example-yellowstone-endpoint

Result
GOOD         Yellowstone gRPC endpoint is ready
Connect      42 ms
Unary        6 passed · 0 failed
Stream       first slot update in 318 ms
Latest slot  424,000,123

Checks
Category         Status    Summary
Transport        PASS      Connected over TLS (HTTP/2)
Authentication   PASS      Token accepted
Unary            PASS      6 / 6 supported checks passed
Stream           PASS      first slot update in 318 ms
Freshness        PASS      Slot stream is active

Tip: run with --verbose to see full details.
```

Exit codes follow the same mapping as the other commands (see
[Exit Codes](#exit-codes)): `0` GOOD, `1` WARNING, `2` BAD, `3` UNKNOWN/error.

### Yellowstone gRPC comparison

Rank two or more Yellowstone gRPC endpoints for a workload, the way `compare`
ranks HTTP RPC endpoints. Each endpoint runs the same safe, slot-only `grpc check`
probe **concurrently**, then endpoints are scored and ranked by verdict, connect
latency, time-to-first-slot-update, slot freshness, and stream stability:

```bash
sol-doctor grpc compare \
  --grpc https://example-yellowstone-a \
  --grpc https://example-yellowstone-b \
  --profile latency
```

Most Yellowstone endpoints require an `x-token`, and different providers use
different tokens. Pair each token with its endpoint by passing `--x-token-env`
once per `--grpc`, in the same order. As with `grpc check`, tokens are read
**only** from environment variables — never from the command line — and are never
printed, serialized, or logged:

```bash
export YELLOWSTONE_A_TOKEN="token-for-a"
export YELLOWSTONE_B_TOKEN="token-for-b"

sol-doctor grpc compare \
  --grpc https://example-yellowstone-a --x-token-env YELLOWSTONE_A_TOKEN \
  --grpc https://example-yellowstone-b --x-token-env YELLOWSTONE_B_TOKEN \
  --profile indexer
```

Pass `--x-token-env` **none** (all endpoints anonymous), **once** (one token
shared by every endpoint), or **once per endpoint** (paired by position). Any
other count is rejected.

| Profile | Optimized for |
| --- | --- |
| `general` | Balanced connect, first-event, and slot freshness. |
| `latency` | Bots/MEV: connect latency and time-to-first-slot-update. |
| `indexer` | Indexers: slot freshness and slot-stream stability. |

`grpc compare` emits concise / `--verbose` human output, `--json` (with a
`schema_version`), and a `--report <PATH>` Markdown report. It subscribes **only**
to slots and is a point-in-time diagnostic, not a benchmark. Because Yellowstone
gRPC does not expose a genesis hash, `grpc compare` cannot detect a mixed-network
comparison the way HTTP `compare` does — compare endpoints on the **same** Solana
network, and slot freshness is ranked relative to the freshest endpoint observed.

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

A real `--verbose` run against `https://api.mainnet-beta.solana.com`, showing
full per-check detail (full redacted URL, per-method latencies, full hashes). The
concise default is shown in the [CLI Preview](#cli-preview) above:

```text
Solana Infra Doctor · RPC Readiness

Target
RPC URL   https://api.mainnet-beta.solana.com/

Result
GOOD         All RPC readiness checks passed
Latency      18 ms average
Checks       11 passed · 0 failed
Block time   16s behind (finalized)
Fee market   median 0 micro-lamports/CU
Token        Token Program ready · Token-2022 ready

Checks

Core
- getHealth       PASS  35 ms  health is ok
- getVersion      PASS  9 ms   solana-core 4.0.0
- getGenesisHash  PASS  24 ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
- getSlot         PASS  5 ms   slot 424282448

Blockhash
- getLatestBlockhash  PASS  2 ms  FzsSsc1FBjsERVk6ZqJpqtCKSBLG7GywRFFNb2yBmLAz
- isBlockhashValid    PASS  5 ms  latest blockhash is valid

Performance
- getRecentPerformanceSamples  PASS  67 ms  214347 transactions across 152 slots in 60s
- getBlockTime                 PASS  21 ms  finalized block time 16s behind wall clock
- getRecentPrioritizationFees  PASS  11 ms  median priority fee 0 micro-lamports/CU (max 0)

Token
- getAccountInfo  PASS  8 ms   Token Program ready: executable 36-byte program owned by BPFLoaderUpgradeab1e11111111111111111111111
- getAccountInfo  PASS  19 ms  Token-2022 ready: executable 36-byte program owned by BPFLoaderUpgradeab1e11111111111111111111111
```

## Compare Output Example

A real `--verbose` `bot`-profile comparison of two public mainnet endpoints, with
full per-endpoint detail. The lower-latency endpoint (#1) is not the winner: #2
serves fresher slots, which the `bot` profile weighs more heavily. (The concise
table is in the [CLI Preview](#cli-preview) above.)

```text
Solana Infra Doctor · RPC Comparison

Profile: bot

RPC #1
URL                   https://api.mainnet-beta.solana.com/
Genesis               5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
Verdict               GOOD
Score                 99/100
Slot                  424282690
Slot lag              32 slots behind
Average latency       18 ms
Block time lag        15s behind
Median priority fee   0 micro-lamports/CU
Token Program         ready
Token-2022            ready
Failed checks         none
Blockhash valid       yes

RPC #2
URL                   https://solana-rpc.publicnode.com/
Genesis               5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
Verdict               GOOD
Score                 100/100
Slot                  424282722
Slot lag              baseline
Average latency       95 ms
Block time lag        2s behind
Median priority fee   0 micro-lamports/CU
Token Program         ready
Token-2022            ready
Failed checks         none
Blockhash valid       yes

Recommendation
Best RPC: #2 · solana-rpc.publicnode.com
RPC #2 is recommended for bot workloads.
RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.
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
- [`examples/terminal/ws-mainnet.txt`](examples/terminal/ws-mainnet.txt)
  — `ws` WebSocket readiness terminal output.
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

## Use in CI (GitHub Action)

Gate a workflow on RPC readiness with the bundled composite action — it installs
`sol-doctor` and runs it, so the job fails when the endpoint is not ready:

```yaml
- name: Check Solana RPC readiness
  uses: satyakwok/solana-infra-doctor@v1
  with:
    rpc: https://api.mainnet-beta.solana.com
    fail-on-warning: "true"
```

Inputs: `command` (`check`/`ws`/`compare`, default `check`), `rpc`,
`fail-on-warning`, `samples`, `timeout-ms`, `json`, `verbose`, `version`, and
`args` (raw passthrough — e.g. extra `--rpc` for `compare`). The job's success
follows the [exit codes](#exit-codes) above, so a `BAD` endpoint (or `WARNING`
with `fail-on-warning`) fails the step. Use the moving major tag `@v1`, or pin a
specific release tag (e.g. `@v0.9.0`) for fully reproducible runs.

> `fail-on-warning` and `samples` apply to `command: check` only.

## Current Limitations

- `check` and `compare` use HTTP JSON-RPC; `sol-doctor ws` covers slot and logs
  subscription readiness (no account/program subscriptions yet).
- `grpc check` (single endpoint) and `grpc compare` (multi-endpoint ranking)
  subscribe **only** to slots; broader subscription diagnostics are not included
  yet. Both are point-in-time diagnostics, not a benchmark or SLA. `grpc compare`
  cannot detect a mixed-network comparison (gRPC exposes no genesis hash), so
  compare endpoints on the same Solana network.
- Scores are deterministic heuristics, not provider guarantees.
- This is a local-first CLI, not a hosted monitoring service.
- Token readiness confirms the SPL Token and Token-2022 program accounts are
  served; transaction simulation and account indexing checks are not covered yet.
- No full Solana or Agave SDK is used; the only Solana crate pulled in is the
  lightweight `solana-pubkey` (transitively, via the gRPC proto definitions).
- No Prometheus exporter, dashboard, hosted cloud service, marketplace, token,
  NFT, points, airdrop, or governance features.

## Security and Privacy

Solana Infra Doctor redacts credentials and likely API keys from displayed RPC
and gRPC URLs, error messages, JSON output, and Markdown reports. Avoid sharing
raw private RPC URLs.

The Yellowstone gRPC `x-token` is read **only** from the environment variable
named by `--x-token-env` (never from a command-line argument) and is never
printed, serialized into JSON, written to a report, or logged. A missing or
empty token variable is reported as a local configuration error before any
connection is attempted.

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

See [`docs/roadmap.md`](docs/roadmap.md) for the full milestone list and scope
boundaries.

**Recently shipped**

- **Yellowstone gRPC endpoint comparison** (`grpc compare`) — rank gRPC endpoints
  by connect latency, time-to-first-event, and slot freshness for a workload
  profile.
- **Yellowstone gRPC readiness check** (`grpc check`).
- Repeat sampling mode (`--samples`) with p50/p95 latency percentiles.
- SPL Token and Token-2022 readiness checks.
- A GitHub Action wrapper and prebuilt binaries (`cargo binstall`) for CI and
  easy install.

**Near-term**

- A Markdown report for `check` (today `compare`, `grpc check`, and `grpc compare`
  emit one) and richer report templates.
- More example reports and localized docs.

**Later**

- Additional WebSocket subscriptions (account/program), beyond slot and logs.
- Optional local benchmark history file.
- A provider comparison playbook.

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
