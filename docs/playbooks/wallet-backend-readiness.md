# Playbook: wallet / backend RPC readiness

## Problem

A wallet or app backend mostly does interactive reads (balances, account info,
recent blockhash) and the occasional send. Users feel **latency** directly, so a
snappy endpoint usually wins — but it still needs valid blockhashes and the token
programs your app touches.

## Recommended command

```bash
sol-doctor check --rpc https://your-rpc --samples 10 --verbose
```

Compare candidates with the `wallet` profile:

```bash
sol-doctor compare --rpc https://rpc-a --rpc https://rpc-b --profile wallet
```

## What to look at

- **Latency `p50` / `p95`** from `--samples` — tail latency is what users notice.
- **Blockhash freshness + validity** — needed for signing/sending.
- **Token / Token-2022 readiness** — that the RPC serves the token programs.
- **Verdict** — `GOOD` with low p95 is the target for interactive UX.

## Common failure cases

- High `p95` with a fine `p50` — intermittent slowness; bad for UX.
- Token check failing — the endpoint may not serve the program account as expected.
- `WARNING` from elevated average latency.

## Notes & safety

- The `wallet` profile favours latency over deep freshness — appropriate for reads,
  not for backfills.
- Point-in-time; run from your backend's region.
- Share only redacted output; never a private URL or key.
