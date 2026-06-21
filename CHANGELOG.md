# Changelog

## 0.14.1 - Unreleased

### Fixed
- Include accepted schemes in `InvalidRpcUrl` error messages so mistyped URLs
  hint at the expected format.

### Documentation / metadata
- List exit codes in `sol-doctor --help` so CI users see them without leaving the
  terminal (`0` GOOD | `1` WARNING | `2` BAD | `3` UNKNOWN).

## 0.14.0 - 2026-06-06

### Added
- A top-level **`schema_version`** field on `check`, `compare`, and `ws` JSON
  output (previously only on the Yellowstone gRPC JSON), so every `--json`
  payload is versioned for forward-compatible consumption.

### Documentation / metadata
- Document the data-readiness (`--data`) output and add real terminal
  screenshots for `check --data` and `compare --data`.
- Add `homepage` and `documentation` to the crate metadata; correct the
  resilience module doc (HTTP 5xx is not retried — only timeouts, connection
  errors, and 429).

## 0.13.0 - 2026-06-05

- Add **`sol-doctor compare --data`**: run the data-readiness checks across every
  endpoint and let the **`indexer` profile score them** — `getProgramAccounts`
  enablement and archival depth now feed the indexer ranking, so `compare` can
  answer *"which provider should I pay for my indexer."* A `gated`
  `getProgramAccounts` materially lowers an endpoint's indexer score and adds an
  advisory note; a `ready` endpoint with full archival is rewarded. The fields
  appear in the JSON and Markdown comparison reports. Off by default; other
  profiles ignore data-readiness.
- Add **`sol-doctor check --data`**: data-readiness checks for indexer and
  data-pipeline workloads, under a new informational `Data` category.
  - **`getProgramAccounts` enablement** — a bounded probe (a `dataSize: 1` filter
    that matches no account, plus `dataSlice` length `0`) detects whether the
    method is enabled without enumerating a large account set. Classified as
    `ready`, `gated` (disabled or the program is excluded from the account
    secondary indexes), or `degraded` (transport failure). A `gated` result is a
    capability fact, not an endpoint failure: it does **not** change the general
    verdict. Pass `--data-program <PUBKEY>` to probe your own program (the default
    is a small, non-excluded program that reflects method enablement).
  - **Archival history depth** — `getFirstAvailableBlock` reports the oldest slot
    the endpoint can serve (`0` = history from genesis / full archival) and the
    depth in slots behind the current slot.
  - Data-capability probes are excluded from the latency average that drives the
    verdict (a slow `getProgramAccounts` does not make a healthy endpoint look
    slow), and the JSON report gains `program_accounts`, `oldest_available_slot`,
    and `archival_depth_slots` fields (present only with `--data`). The checks are
    off by default; `--data` adds two extra requests.

## 0.12.0 - 2026-06-05

- Add **`sol-doctor grpc compare`**: rank multiple Yellowstone gRPC endpoints for
  a workload profile. It reuses the single-endpoint `grpc check` probe (safe by
  default, slot-only, redaction-safe) for every endpoint **concurrently**, then
  scores and ranks them by verdict, connect latency, time-to-first-slot-update,
  slot freshness, and stream stability, and recommends a best/worst endpoint with
  an honest connect-versus-stream tradeoff note.
  - **Profiles** (`--profile`): `general` (balanced), `latency` (bots/MEV — weighs
    connect and first-event), and `indexer` (weighs slot freshness and stream
    stability).
  - **Per-endpoint tokens**: pass `--grpc` more than once and pair `--x-token-env`
    by position — provide none (all anonymous), one (shared across endpoints), or
    one per endpoint. As with `grpc check`, the token is read **only** from an
    environment variable, never from the command line, and is never printed,
    serialized, or logged.
  - Output is available as concise / `--verbose` human text, `--json` (with a
    `schema_version`), and a `--report <PATH>` Markdown report.
  - gRPC does not expose a genesis hash, so `grpc compare` cannot detect a
    mixed-network comparison; endpoints are assumed to be on the same Solana
    network (documented), and slot freshness is ranked relative to the freshest
    endpoint observed.

## 0.11.0 - 2026-06-04

- Add **`sol-doctor grpc check`**: Yellowstone gRPC readiness diagnostics for
  backend workloads. The command validates and redacts the gRPC URL, connects
  (TLS + HTTP/2 for `https`), optionally authenticates with an `x-token` read
  **only** from an environment variable (`--x-token-env`), runs safe unary
  probes (`Ping`, `GetVersion`, `GetSlot`, `GetBlockHeight`, `GetLatestBlockhash`,
  `IsBlockhashValid`), and opens a narrow **slot-only** `Subscribe` stream to
  measure time-to-first-slot-update and the latest observed slot. An optional
  `--rpc` cross-check compares the gRPC slot against an HTTP RPC slot. Output is
  available as concise/`--verbose` human text, `--json` (with a `schema_version`),
  and a `--report <PATH>` Markdown report. A new gRPC error-kind taxonomy
  (`invalid_grpc_url`, `tls_error`, `unauthenticated`, `unimplemented`,
  `no_first_event`, `stream_closed`, …) and per-failure remediation hints are
  included.
- **Safety:** the command is safe by default — it never sends transactions or
  modifies remote state, never subscribes to accounts/transactions/blocks (only
  slots), bounds every connection/request/stream with a deadline, and never
  prints, serializes, or logs the token.
- **Dependencies:** adds `tonic` 0.14 and `yellowstone-grpc-proto` 12.4 (which
  pulls only `solana-pubkey`, not the full Solana/Agave SDK; HTTP/2 stack is
  shared with the existing `reqwest` dependency). Raises the MSRV to **1.88**
  (required by `tonic` 0.14). The supported release-target matrix (Linux gnu +
  musl, macOS Intel + Apple Silicon, Windows) is unchanged; no system `protoc`
  is required (a vendored `protoc` is used at build time).
- Existing commands (`check`, `compare`, `ws`) are unchanged.

## 0.10.0 - 2026-06-04

- Attach **prebuilt `sol-doctor` binaries** to each GitHub Release and add
  `cargo binstall` support, so the CLI can be installed without compiling from
  source. A new `release-binaries` workflow builds per-target archives
  (`sol-doctor-<target>`) on each pushed `v*` tag, and
  `[package.metadata.binstall]` points `cargo binstall solana-infra-doctor` at
  them (zip on Windows, tgz elsewhere). No crate runtime, CLI, output, or
  JSON-shape changes.
- Add a **composite GitHub Action** (`action.yml`) that installs `sol-doctor`
  via `cargo binstall` and runs it, so a workflow can gate a job on Solana RPC
  readiness using the verdict exit code (`0` GOOD / `1` WARNING / `2` BAD /
  `3` UNKNOWN). Inputs cover `command` (`check`/`ws`/`compare`), `rpc`,
  `fail-on-warning`, `samples`, `timeout-ms`, `json`, `verbose`, `version`, and
  a raw `args` passthrough; every input is passed through the environment
  (never interpolated into the shell body) to prevent injection. A self-test
  workflow exercises the action against a public RPC.
- Gate the action's `--fail-on-warning` and `--samples` inputs on
  `command: check` — they do not exist on `ws`/`compare`, where they would be a
  parse error — and document the moving `@v1` tag alongside pinned release tags.

## 0.8.0 - 2026-06-04

- Add **SPL Token Program** and **Token-2022** readiness checks: a new `Token`
  diagnostic category probes the canonical token program accounts
  (`Tokenkeg…` and `TokenzQd…`) via `getAccountInfo`. An endpoint is "ready"
  when it serves each account as an `executable` program with non-zero data
  length, confirming the RPC can back token-touching workloads (wallets,
  trading bots, token indexers).
- These checks are **informational**: a failure caps the verdict at `WARNING`
  rather than `BAD`. Profile scoring rewards token readiness for the `wallet`,
  `bot`, and `indexer` profiles (neutral for `general`/`ci`), and those profiles
  emit advisory notes when a program account is unavailable.
- Readiness is surfaced in the `check` Result block and `Token` category table,
  the verbose `compare` detail, JSON (`token_program_ready`, `token_2022_ready`),
  and the Markdown report. No `solana-sdk` dependency is added; account data
  length is derived from `space` with a hardened base64 fallback.

## 0.7.0 - 2026-06-04

- Add two new diagnostic checks: **`getBlockTime`** (on the latest finalized
  slot) yields how far the finalized chain tip lags wall clock — a **freshness**
  signal — and **`getRecentPrioritizationFees`** surfaces the median recent
  priority fee as **fee-market context**. (We use `getBlockTime` rather than
  `getBlock`, which returns "Block not available" for recent slots on public RPC.)
- Improve scoring: **block-time freshness** is now a scoring signal (a stale
  finalized tip scores lower; indexers penalize it more and note it).
  Prioritization fees are chain-wide, so they are surfaced as context but do not
  affect the per-endpoint score. New fields appear in human output, JSON
  (`block_time_lag_secs`, `prioritization_fee_median`), and the Markdown report.
  `getFeeForMessage` was intentionally not added (it requires constructing a
  signed message; low marginal value).
- Make non-critical (informational) check failures cap the verdict at `WARNING`
  rather than escalating to `BAD`. Previously two or more non-critical failures
  forced `BAD`; with the new informational checks that was too harsh, so an
  endpoint that serves core/blockhash but not, say, `getBlockTime` and
  `getRecentPrioritizationFees` is now `WARNING`. Critical failures, repeated
  timeouts, and excessive latency still yield `BAD`.
- Make the `ws` diagnostic **reconnect with exponential backoff** when a
  connection fails to establish or drops before the first notification (up to 3
  reconnects). A connected-but-quiet endpoint is not retried. The number of
  reconnects is reported (`reconnect_attempts`) and noted in human output.
- Support multiple PubSub subscription types via `ws --subscription slot|logs`
  (default `slot`, backward compatible), built on an extensible `Subscription`
  type. `logsSubscribe` is now testable end to end.
- Generalize the WebSocket "first notification" step so it passes for any
  subscription (slot subscriptions also report the slot; log subscriptions do
  not). Reconnect/retry paths never log the URL, preserving redaction.
- Run `compare` endpoint checks **concurrently** instead of sequentially, so the
  total time is bounded by the slowest endpoint rather than their sum. Endpoint
  order and the first-error behavior are unchanged.
- Add per-endpoint **resilience** to the HTTP RPC client: a token-bucket rate
  limiter (politeness toward public RPCs, mainly for heavy `--samples` runs) and
  exponential-backoff retry on transient failures (timeouts, connection errors,
  HTTP 429). Retries never log the URL, preserving redaction. Built on the
  dependency-free `reliakit-backoff` / `reliakit-ratelimit` crates.
- Raise the minimum supported Rust version (MSRV) to 1.85 (required by the
  reliakit crates). No CLI, output, or JSON-shape changes.

## 0.6.0 - 2026-06-04

- Add `check --samples <N>`: probe round-trip latency `N` times (lightweight
  `getHealth` calls) and report `p50`/`p95` percentiles (plus `min`/`max` under
  `--verbose`), since a single sample hides tail latency. Adds a `Samples` line
  to human output and an additive `latency_samples` object to JSON. The default
  is a single sample, and the flag does not change the verdict, scoring, or exit
  codes.

## 0.5.1 - 2026-06-04

- Fix the README CLI screenshots, which were captured with a proportional font
  and appeared to have misaligned columns. The renderer already aligns columns
  from plain (unstyled) text; the screenshot capture now pins an explicit
  monospace font and the images are regenerated from real live runs. No
  diagnostic, scoring, exit-code, JSON, or Markdown behavior changes.
- Add deterministic column-alignment tests (ANSI-stripped, with color enabled
  and disabled).
- Add a manually triggered crates.io Trusted Publishing workflow
  (`.github/workflows/publish-crates.yml`) that authenticates with a short-lived
  OIDC token instead of a stored API token, and a maintainer release guide
  (`docs/releasing.md`).

## 0.5.0 - 2026-06-04

- Redesign human terminal output into a concise, scannable default: a per-category
  summary for `check`, a one-row-per-endpoint summary table for `compare`, and a
  compact step table for `ws`. Drop decorative rules; use whitespace and headings.
- Add a global `-v`/`--verbose` flag that expands human output with full per-check
  detail (full redacted URL, per-method latencies, full hashes, per-endpoint
  detail, notes). It affects human output only; `--json` is unchanged and takes
  precedence.
- Hide full genesis hashes, blockhashes, and full URLs in the concise view
  (showing a safe hostname label); the full redacted URL and hashes remain
  available with `--verbose` and in JSON.
- Standardize the status vocabulary: overall `GOOD`/`WARNING`/`BAD`/`UNKNOWN`,
  per-check `PASS`/`WARN`/`FAIL`/`SKIP`; format units with a space (`13 ms`),
  use sentence-case summaries, and prefer "First notification" wording for the
  WebSocket time-to-first-event step.
- Honor `TERM=dumb` (in addition to `NO_COLOR`) when resolving `--color auto`.
- Add the [CLI Output Guide](docs/cli-output.md) and
  [screenshot reproducibility notes](docs/readme-screenshots.md), a
  `scripts/capture-readme-screenshots.sh` helper, and refresh the README preview
  with real, live screenshots of the new output.
- Documentation only: no change to diagnostic logic, scoring, ranking, exit
  codes, or the JSON/Markdown shape (summary wording values are updated to match
  the human output).

## 0.4.0 - 2026-06-04

- Add TTY-aware, semantic color to human terminal output for `check`, `compare`,
  and `ws`: verdicts and `OK`/`FAIL` markers carry status colors, labels are
  muted, and titles are emphasized, using a restrained truecolor palette.
- Add the global `--color auto|always|never` flag (default `auto`), auto-detect
  whether stdout is a terminal, and honor the `NO_COLOR` environment variable.
- Never colorize `--json` output; keep non-TTY / piped output byte-for-byte
  identical to the previous uncolored output.

## 0.3.0 - 2026-06-04

- Reorganize the diagnostic engine into clean internal modules (`checks`,
  `compare` with `scoring`/`render`, `rpc` with `models`, `ws` with `analysis`,
  `checks::verdict`) and strengthen `src/lib.rs` so the engine can be reused or
  extracted into a separate core crate later.
- Preserve the `solana-infra-doctor` crate name, the `sol-doctor` binary, the
  CLI behavior, JSON shape, Markdown report, and redaction behavior.
- Add a README "Demo" section with real terminal screenshots for `check`,
  `compare`, and `ws`.
- Add a real `bot`-profile comparison report example.
- Add a "How to vet a Solana RPC endpoint" guide under `docs/`.
- Exclude demo screenshots from the published crate to keep it small.

## 0.2.2 - 2026-06-03

- Strengthen README positioning for Solana RPC readiness diagnostics.
- Document WebSocket diagnostics and redaction-safe reporting more clearly.
- Add practical use cases for RPC provider comparison, CI checks, and technical reports.
- Add a docs guide for producing an RPC readiness report.
- Update the package description and CLI help text to mention WebSocket checks.
- Fix stale README wording from earlier releases.

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
