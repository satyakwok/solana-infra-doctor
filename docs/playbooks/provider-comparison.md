# Playbook: comparing RPC providers

## Problem

You're choosing between RPC providers (or your own nodes) and want a neutral,
data-driven read instead of vibes — for a specific workload, not "fastest overall."

## Recommended command

```bash
sol-doctor compare \
  --rpc https://rpc-a \
  --rpc https://rpc-b \
  --rpc https://rpc-c \
  --profile <general|wallet|bot|indexer|ci> --verbose
```

Run it once per profile you care about — the winner can change by workload.

## What to look at

- **Score (0–100)** and **verdict** per endpoint; the recommended pick for the
  profile.
- **Slot lag** (freshness) vs **average latency** — the core trade-off. The fastest
  endpoint is not automatically the best (freshness can win for bots/indexers).
- **Failed checks** and **blockhash valid** per endpoint.
- Add `--data` for the `indexer` profile to factor in `getProgramAccounts`/archival.

## Common failure cases

- Provider needs an API key — public no-auth endpoints are limited; key-gated
  providers (Helius, QuickNode, Triton, Ankr, dRPC) need an account to test fairly.
- A `network_mismatch` note — endpoints returned different genesis hashes (different
  clusters); ranking is disabled until they match.
- Rankings differ run-to-run — latency/freshness drift; sample a few times.

## Notes & safety

- This is **not** an authoritative ranking or an SLA — it's a point-in-time snapshot
  from one location against the endpoints you pass. Latency is region-dependent.
- All endpoints can score `GOOD` and still differ — frame results as *fit for
  workload*, not "good vs bad provider."
- Share the redacted `--report` Markdown; never paste a private URL or key. See
  [`docs/contributing/real-run-evidence.md`](../contributing/real-run-evidence.md).
