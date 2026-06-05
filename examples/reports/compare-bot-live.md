# Solana Infra Doctor RPC Compare Report

Profile: `bot`

## Summary

- Best RPC: RPC #2
- Worst RPC: RPC #1

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 83/100 | 424115192 | 33 slots behind | 12ms | none | yes |
| RPC #2 | `https://solana-rpc.publicnode.com/` | `GOOD` | 90/100 | 424115225 | baseline | 110ms | none | yes |

## Per-Endpoint Details

### RPC #1

- URL: `https://api.mainnet-beta.solana.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 83/100
- Slot: 424115192
- Slot lag: 33 slots behind
- Average latency: 12ms
- Failed checks: none
- Notes: none

### RPC #2

- URL: `https://solana-rpc.publicnode.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 90/100
- Slot: 424115225
- Slot lag: baseline
- Average latency: 110ms
- Failed checks: none
- Notes: none

## Recommendation

Best RPC: RPC #2

Worst RPC: RPC #1

RPC #2 is recommended for bot workloads.

RPC #1 has lower latency, but RPC #2 is fresher. For bot workloads, slot freshness may matter more than raw HTTP latency.

## Limitations

- Compare uses HTTP JSON-RPC diagnostics; run `sol-doctor ws` for WebSocket readiness.
- Endpoints are checked concurrently; the run is bounded by the slowest endpoint.
- Scores are deterministic heuristics, not a provider guarantee.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
