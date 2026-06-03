# Producing an RPC Readiness Report

This guide shows how to use `sol-doctor` to produce a short, redaction-safe
diagnostic report for a set of Solana RPC endpoints. It is a workflow guide, not
a paid service or an SLA — `sol-doctor` is a local diagnostic CLI.

## Inputs required

- One or more RPC HTTP URLs to evaluate (the same Solana network — for example
  all mainnet-beta).
- The workload the endpoints will serve (`general`, `wallet`, `bot`, `indexer`,
  or `ci`).
- Optionally, the WebSocket URL if the provider uses a separate host.

Keep private/API-key URLs out of anything you share — `sol-doctor` redacts them
in its own output, but the raw command line you type is not redacted.

## Commands to run

Single-endpoint readiness:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Compare candidates for the target workload and write a Markdown report:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --report rpc-report.md
```

WebSocket readiness for realtime workloads:

```bash
sol-doctor ws --rpc https://api.mainnet-beta.solana.com
```

Machine-readable output for CI or for attaching to a report:

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com \
  --profile bot \
  --json
```

## What the output means

- **Verdict** — `GOOD` / `WARNING` / `BAD` / `UNKNOWN` (exit codes `0`/`1`/`2`/`3`).
- **Score (0–100)** — a deterministic heuristic per endpoint for the chosen
  profile. It is a readiness signal, not a benchmark or a provider guarantee.
- **Average latency** — mean per-request HTTP latency observed during the run.
- **Slot / slot lag** — how far an endpoint is behind the freshest endpoint in
  the comparison (within the same network).
- **Failed checks** — JSON-RPC methods that did not succeed.
- **Best / worst endpoint** — the ranking for the selected profile.

## Interpreting latency vs slot freshness

Lower latency is not automatically better. An endpoint can answer quickly but
serve **stale** slot data, while a slightly slower endpoint stays **fresher**.
For bot and indexer workloads, slot freshness often matters more than raw HTTP
latency. The recommendation text calls out this tradeoff when the lower-ranked
endpoint is faster but staler.

## What WebSocket readiness means

`sol-doctor ws` connects, subscribes with `slotSubscribe`, measures the
**time-to-first-slot-notification**, then unsubscribes and closes. It is a
single-shot readiness check — it confirms that realtime subscriptions work and
how quickly the first event arrives. It is **not** continuous monitoring.

## Privacy / redaction note

Credentials and likely API keys are redacted from terminal output, JSON, the
Markdown report, and error messages (basic-auth, common secret query parameters,
and provider tokens in URL paths). Always confirm a generated report does not
contain a secret before sharing it.

## Limitations

- HTTP `check`/`compare` are single-shot; latency is a snapshot, not a
  distribution. Run repeatedly for a fuller picture.
- Compare only ranks endpoints on the **same** Solana network (mixed networks
  are rejected by genesis hash).
- Scores are deterministic heuristics, not provider guarantees.
- WebSocket readiness covers slot subscriptions only.

## Sample deliverable outline

A short readiness report can be structured as:

1. Scope — endpoints evaluated, target workload, date.
2. Summary verdict and recommended endpoint.
3. Comparison table (from the generated Markdown report).
4. Latency vs slot-freshness tradeoff notes.
5. WebSocket readiness result.
6. Limitations and disclaimer.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated
with or endorsed by Solana Foundation.
