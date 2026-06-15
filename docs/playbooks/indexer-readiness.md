# Playbook: indexer RPC readiness

## Problem

An indexer or data pipeline scans program accounts and often backfills history. It
needs `getProgramAccounts` to be **enabled** (many providers gate it) and enough
**archival depth** to reach the blocks it cares about. Freshness matters; raw
latency matters less than for interactive apps.

## Recommended command

```bash
sol-doctor check --rpc https://your-rpc --data --verbose
```

Rank candidates with the `indexer` profile + data-readiness:

```bash
sol-doctor compare \
  --rpc https://rpc-a --rpc https://rpc-b \
  --profile indexer --data --verbose
```

To probe `getProgramAccounts` on *your* program:

```bash
sol-doctor check --rpc https://your-rpc --data --data-program <YOUR_PROGRAM_ID>
```

## What to look at

- **`getProgramAccounts`** — `ready`, `gated`, or `degraded`. `gated` means the
  endpoint disables/restricts it; an indexer that scans program accounts can't use it.
- **Archival** — `full (from genesis)` vs `from slot N` (recent-only); shallow
  history limits deep backfills.
- **Slot freshness** — the indexer should track the tip.

## Common failure cases

- `getProgramAccounts` gated — common on shared public endpoints.
- Archival `from slot N` when you need older blocks — pick a deeper endpoint.
- A huge program (e.g. SPL Token) returns `-32010` (excluded from secondary
  indexes) — that's about the *target program*, not the endpoint; use `--data-program`
  with a small program to test enablement, then your own program for capability.

## Notes & safety

- Data-readiness checks are informational — they don't by themselves flip the
  GOOD/BAD verdict, but they drive `indexer`-profile scoring.
- Point-in-time; share only redacted output.
