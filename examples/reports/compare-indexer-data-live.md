# Solana Infra Doctor RPC Compare Report

Profile: `indexer`

## Summary

- Best RPC: RPC #1
- Worst RPC: RPC #2

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 100/100 | 424533921 | 30 slots behind | 4ms | none | yes |
| RPC #2 | `https://solana-rpc.publicnode.com/` | `GOOD` | 100/100 | 424533951 | baseline | 94ms | none | yes |

## Per-Endpoint Details

### RPC #1

- URL: `https://api.mainnet-beta.solana.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 100/100
- Slot: 424533921
- Slot lag: 30 slots behind
- Average latency: 4ms
- Block time lag: 14s behind
- Median priority fee: 0 micro-lamports/CU
- Token Program: ready
- Token-2022: ready
- getProgramAccounts: ready
- Archival: full (from genesis)
- Failed checks: none
- Notes: none

### RPC #2

- URL: `https://solana-rpc.publicnode.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 100/100
- Slot: 424533951
- Slot lag: baseline
- Average latency: 94ms
- Block time lag: 2s behind
- Median priority fee: 0 micro-lamports/CU
- Token Program: ready
- Token-2022: ready
- getProgramAccounts: ready
- Archival: from slot 423938034
- Failed checks: none
- Notes:
  - Endpoint serves only recent history (not full archival); deep backfill may be limited.

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
