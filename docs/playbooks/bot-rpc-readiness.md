# Playbook: bot RPC readiness

## Problem

A trading/MEV bot reads recent on-chain state and submits transactions. The
lowest-latency RPC is not automatically the best — a fast endpoint that lags the
chain tip serves **stale reads** (prices/balances that already moved). For bots,
slot freshness matters alongside latency.

## Recommended command

```bash
sol-doctor compare \
  --rpc https://your-rpc-a \
  --rpc https://your-rpc-b \
  --profile bot --verbose
```

The `bot` profile weights slot freshness, not just raw latency.

## What to look at

- **Verdict + score (0–100)** per endpoint, and the recommended pick.
- **Slot lag** — `baseline` is the freshest; "N slots behind" means it trails.
- **Average latency** — still matters, but freshness can outweigh it here.
- **Blockhash valid** — a stale/invalid blockhash hurts transaction landing.

## Common failure cases

- A fast endpoint ranked *below* a slower one — expected: it was less fresh.
- `429 Too Many Requests` — the endpoint is rate-limiting your probes (a real
  signal for a high-throughput bot); the tool retries transient cases.
- `WARNING` on elevated latency or a non-critical failed check.

## Notes & safety

- Slot lag is a **read-freshness** signal. Transaction *landing* also depends on
  blockhash validity and leader proximity — a separate axis.
- Results are point-in-time and region-dependent; re-run from where your bot runs.
- To share output: paste the redacted terminal/`--json`/`--report`. Never paste a
  private URL or key — `sol-doctor` redacts URLs, but review before sharing.
