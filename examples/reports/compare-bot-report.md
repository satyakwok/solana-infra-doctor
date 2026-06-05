# Solana Infra Doctor RPC Compare Report

Profile: `bot`

## Summary

- Best RPC: RPC #2
- Worst RPC: RPC #1

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `WARNING` | 20/100 | 312456740 | 65 slots behind | 820ms | none | yes |
| RPC #2 | `https://redacted-rpc.example.com/` | `GOOD` | 90/100 | 312456805 | baseline | 90ms | none | yes |

## Per-Endpoint Details

### RPC #1

- URL: `https://api.mainnet-beta.solana.com/`
- Verdict: `WARNING`
- Score: 20/100
- Slot: 312456740
- Slot lag: 65 slots behind
- Average latency: 820ms
- Failed checks: none
- Notes:
  - Average latency is high for latency-sensitive bot workloads.
  - Slot lag is high for slot-sensitive bot workloads.

### RPC #2

- URL: `https://redacted-rpc.example.com/`
- Verdict: `GOOD`
- Score: 90/100
- Slot: 312456805
- Slot lag: baseline
- Average latency: 90ms
- Failed checks: none
- Notes: none

## Recommendation

Best RPC: RPC #2

Worst RPC: RPC #1

RPC #2 is recommended for bot workloads.

Avoid RPC #1 for latency-sensitive or slot-sensitive workloads.

## Limitations

- Compare uses HTTP JSON-RPC diagnostics; run `sol-doctor ws` for WebSocket readiness.
- Endpoints are checked concurrently; the run is bounded by the slowest endpoint.
- Scores are deterministic heuristics, not a provider guarantee.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
