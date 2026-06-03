# Mixed-Network Comparison Rejection

Compare mode only ranks endpoints on the **same** Solana network. When two
endpoints return different genesis hashes (for example mainnet-beta vs devnet),
slot lag and ranking are not meaningful, so the comparison is rejected.

## Command

```bash
sol-doctor compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://api.devnet.solana.com \
  --profile bot
```

## Output (illustrative)

Slot numbers below are illustrative and will differ on each run.

```text
Solana Infra Doctor — RPC Compare

Profile: bot

Cannot compare endpoints from different Solana networks.
Endpoints returned different genesis hashes; ranking and slot lag are disabled.

RPC #1
URL: https://api.mainnet-beta.solana.com/
Genesis: 5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d
Verdict: GOOD
Score: 75/100
Slot: 424000000
Slot lag: n/a
Average latency: 16ms
Failed checks: none
Blockhash valid: yes

RPC #2
URL: https://api.devnet.solana.com/
Genesis: EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG
Verdict: GOOD
Score: 75/100
Slot: 466000000
Slot lag: n/a
Average latency: 11ms
Failed checks: none
Blockhash valid: yes

Recommendation:
Cannot compare endpoints from different Solana networks.
Slot lag and ranking are disabled because the endpoints report different genesis hashes.
Re-run compare with endpoints on the same network.
```

The process exits with code `3` (`UNKNOWN`) because no reliable cross-network
ranking can be produced.

## Why this matters

Mainnet-beta and devnet have unrelated slot counters, so a raw "slot lag"
between them would be a large, meaningless number. Rejecting the comparison
avoids presenting a misleading ranking.
