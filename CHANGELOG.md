# Changelog

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
