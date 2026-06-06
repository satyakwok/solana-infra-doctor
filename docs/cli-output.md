# CLI Output Guide

Solana Infra Doctor produces three kinds of output for three different audiences:

| Output | Audience | Flag |
| --- | --- | --- |
| Human terminal text | people reading a terminal | default |
| JSON | automation, CI, other programs | `--json` |
| Markdown report | a shareable artifact (PR, ticket, email) | `--report <path>` (compare) |

> Automation should consume `--json`, **not** parse the human terminal text. The
> human layout is tuned for readability and may change between releases; the JSON
> shape is the stable contract.

## Human output

By default, human output is **concise**: it answers "is this endpoint ready?"
at a glance without scrolling.

```text
Solana Infra Doctor · RPC Readiness

Target
Endpoint   api.mainnet-beta.solana.com

Result
GOOD       All RPC readiness checks passed
Latency    13 ms average
Checks     7 passed · 0 failed

Checks
Category       Status    Summary
Core           PASS      4 / 4
Blockhash      PASS      2 / 2
Performance    PASS      1 / 1

Tip: run with --verbose to see full details.
```

`compare` defaults to a one-row-per-endpoint summary table, and `ws` to a compact
step table. See [README CLI Preview](../README.md#cli-preview) for live screenshots.

The summary tables align columns with spaces, so they assume a **monospace
terminal font** — in a proportional font the columns will appear to drift even
though the text is correctly aligned. Verbose output may also wrap depending on
terminal width. This is another reason automation should consume `--json` rather
than parse the human text: the layout targets a human reading a monospace
terminal, while JSON is the stable contract.

### `--verbose` (`-v`)

`--verbose` expands the human output with full per-check detail: the full
(redacted) RPC/WebSocket URL, every method's latency, full genesis hashes and
blockhashes, per-endpoint detail in compare mode, and diagnostic notes.

`--verbose` affects **human output only**. It does not change exit codes, and
when combined with `--json` the JSON output is unchanged (JSON takes precedence —
`--verbose` is ignored for JSON).

### `--samples <N>` (check)

`check --samples <N>` probes round-trip latency `N` times (lightweight
`getHealth` calls) and reports percentiles, since a single sample hides tail
latency. It adds a `Samples` line to human output (`p50 … · p95 …`, plus
`min`/`max` under `--verbose`) and a `latency_samples` object to JSON. The
default is a single sample; `--samples` is presentation/diagnostic only and does
not change the verdict.

### `--color auto|always|never`

- `auto` (default) — color only when stdout is a terminal. It is automatically
  disabled when `NO_COLOR` is set, when `TERM=dumb`, and for `--json`.
- `always` — force color even when piped (used to capture screenshots).
- `never` — never emit ANSI escape codes.

Color is **semantic**: status colors carry meaning, secondary labels are muted,
and section titles are emphasized. When color is disabled the output is
byte-for-byte identical to the colored output with the codes removed — so piping
or redirecting never leaves escape codes in the text.

## Status vocabulary

Two distinct vocabularies are used consistently:

**Overall verdict** (one per command, drives the exit code):

| Verdict | Exit code | Meaning |
| --- | --- | --- |
| `GOOD` | 0 | Required checks passed and latency is acceptable. |
| `WARNING` | 1 | Reachable but degraded (elevated latency or a non-critical failure). |
| `BAD` | 2 | Unreachable, a critical check failed, or latency is too high. |
| `UNKNOWN` | 3 | Not enough data for a reliable verdict. |

**Per-check / per-category status:**

| Status | Meaning |
| --- | --- |
| `PASS` | The check (or every check in a category) succeeded. |
| `WARN` | Only non-critical checks failed; usable but degraded. |
| `FAIL` | A critical check failed. |
| `SKIP` | The check was not run. |

## JSON output

`--json` emits the machine-readable report. It never contains ANSI codes or
human-only layout, applies the same redaction as human output, and keeps a
stable field shape. Prefer it for any programmatic use, CI gates, or alerting.

## Markdown report

`sol-doctor compare --report <path>` and `sol-doctor grpc check --report <path>`
write a shareable Markdown report. It contains no ANSI codes, applies the same
redaction, and is meant to be attached to a PR, ticket, or email. The gRPC report
carries a point-in-time diagnostic disclaimer.

## Data-readiness output (`--data`)

`sol-doctor check --data` adds a **Data** category for indexer and data-pipeline
workloads, surfaced as a `Data` roll-up line (e.g. `getProgramAccounts ready ·
history full (from genesis)`) and two checks:

- **`getProgramAccounts` enablement** — probed against a small, non-excluded
  program (`ComputeBudget…` by default; override with `--data-program <PROGRAM>`
  to test your own). Reported as `ready`, `gated` (the endpoint disables or
  restricts `getProgramAccounts`), or `degraded` (the probe could not complete).
  Large programs such as the SPL Token program are excluded from validator
  secondary indexes on most providers, so they are intentionally *not* used as
  the probe target.
- **Archival history depth** — from `getFirstAvailableBlock`: `full (from
  genesis)` when the oldest available slot is `0`, otherwise `from slot <N>`
  (shallow/recent-only history), which matters for deep backfills.

The Data category is **informational**: it does not by itself flip the overall
`GOOD`/`WARNING`/`BAD` verdict, and its latency is excluded from the latency
average. `sol-doctor compare --data` feeds the same signals into the `indexer`
profile score, so endpoints that gate `getProgramAccounts` or serve only shallow
history rank lower for indexer workloads (`--verbose` shows the per-endpoint
`getProgramAccounts` and `Archival` rows).

## Yellowstone gRPC output (`grpc check`)

`sol-doctor grpc check` uses the same output system as the other commands: a
concise default, a `--verbose` expansion, `--json`, and a `--report` Markdown
file, all sharing the status vocabulary above (`PASS`/`WARN`/`FAIL`/`SKIP` and
the `GOOD`/`WARNING`/`BAD`/`UNKNOWN` verdict).

- **Concise:** a result roll-up (verdict, connect latency, unary pass/fail count,
  stream first-event time, latest slot) plus a per-category table
  (`Transport`, `Authentication`, `Unary`, `Stream`, `Freshness`, and
  `Cross-check` when `--rpc` is supplied).
- **`--verbose`:** the full redacted gRPC URL, per-method unary detail with
  latency and the classified error kind, the HTTP RPC cross-check, warnings, and
  remediation hints.
- **`--json`:** includes a `schema_version`, gRPC-specific `error_kinds`, the
  per-method `unary` results, the `stream` result, and the optional `rpc_slot` /
  `slot_difference`. It **never** contains the `x-token`.

The Yellowstone `x-token` is read only from the environment (`--x-token-env`) and
is never printed, serialized, written to a report, or logged.

## Why URLs are redacted and long hashes are hidden by default

- **Redaction:** credentials and likely API keys in RPC/WebSocket URLs are
  redacted everywhere they could appear — default output, verbose output, errors,
  JSON, and Markdown — so a screenshot or log never leaks a secret. Default human
  output shows only a safe hostname label; `--verbose` shows the full *redacted*
  URL.
- **Hidden hashes:** genesis hashes and blockhashes are diagnostic detail, not
  summary information. Hiding them by default keeps the summary scannable; they
  are available under `--verbose` and in JSON.

## Why values vary, and what this is not

Latency, slot numbers, and slot lag are live measurements from a single run, from
this machine's vantage point. They vary by time of day, region, network path, and
endpoint load. A run is a **point-in-time diagnostic snapshot from one vantage
point — not an SLA, an uptime guarantee, or a provider benchmark.** Scores are
deterministic heuristics for comparing endpoints for a workload, not a guarantee
of provider behavior.
