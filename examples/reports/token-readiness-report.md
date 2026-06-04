# Solana Infra Doctor RPC Compare Report

Profile: `wallet`

## Summary

- Best RPC: RPC #1
- Worst RPC: RPC #2

## Comparison

| RPC | URL | Verdict | Score | Slot | Slot lag | Average latency | Failed checks | Blockhash valid |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| RPC #1 | `https://api.mainnet-beta.solana.com/` | `GOOD` | 100/100 | 424206924 | 32 slots behind | 9ms | none | yes |
| RPC #2 | `https://solana-rpc.publicnode.com/` | `GOOD` | 100/100 | 424206956 | baseline | 128ms | none | yes |

## Per-Endpoint Details

### RPC #1

- URL: `https://api.mainnet-beta.solana.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 100/100
- Slot: 424206924
- Slot lag: 32 slots behind
- Average latency: 9ms
- Block time lag: 14s behind
- Median priority fee: 0 micro-lamports/CU
- Token Program: ready
- Token-2022: ready
- Failed checks: none
- Notes: none

### RPC #2

- URL: `https://solana-rpc.publicnode.com/`
- Genesis: `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d`
- Verdict: `GOOD`
- Score: 100/100
- Slot: 424206956
- Slot lag: baseline
- Average latency: 128ms
- Block time lag: 2s behind
- Median priority fee: 0 micro-lamports/CU
- Token Program: ready
- Token-2022: ready
- Failed checks: none
- Notes: none

## Recommendation

Best RPC: RPC #1

Worst RPC: RPC #2

RPC #1 is recommended for wallet workloads.

Avoid RPC #2 for wallet transaction flows if blockhash or core checks failed.

## Limitations

- Compare uses HTTP JSON-RPC diagnostics; run `sol-doctor ws` for WebSocket readiness.
- Checks run sequentially for deterministic v0.1 behavior.
- Scores are deterministic heuristics, not a provider guarantee.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
