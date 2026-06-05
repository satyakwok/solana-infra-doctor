# Solana Infra Doctor RPC Compare Report

Profile: `indexer`

## Summary

- Best RPC: RPC #1
- Worst RPC: RPC #2

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 90/100 | 312456800 | baseline | 140ms | none | yes |
| RPC #2 | `https://redacted-rpc.example.com/` | `WARNING` | 25/100 | 312456120 | 680 slots behind | 260ms | getRecentPerformanceSamples | yes |

## Per-Endpoint Details

### RPC #1

- URL: `https://api.mainnet-beta.solana.com/`
- Verdict: `GOOD`
- Score: 90/100
- Slot: 312456800
- Slot lag: baseline
- Average latency: 140ms
- Failed checks: none
- Notes: none

### RPC #2

- URL: `https://redacted-rpc.example.com/`
- Verdict: `WARNING`
- Score: 25/100
- Slot: 312456120
- Slot lag: 680 slots behind
- Average latency: 260ms
- Failed checks: getRecentPerformanceSamples
- Notes:
  - Slot lag is high for indexer catch-up and freshness.
  - Recent performance samples are unavailable for indexer diagnostics.

## Recommendation

Best RPC: RPC #1

Worst RPC: RPC #2

RPC #1 is recommended for indexer workloads.

Avoid RPC #2 for freshness-sensitive indexer workloads.

## Limitations

- Compare uses HTTP JSON-RPC diagnostics; run `sol-doctor ws` for WebSocket readiness.
- Endpoints are checked concurrently; the run is bounded by the slowest endpoint.
- Scores are deterministic heuristics, not a provider guarantee.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
