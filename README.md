# Solana Infra Doctor

Solana Infra Doctor is a lightweight Rust CLI for diagnosing Solana RPC and infrastructure health.

It checks whether an RPC endpoint is not only online, but usable for builders, bots, wallets, indexers, and production applications. The v0.1 scope is intentionally small: validate an HTTP RPC URL, run core JSON-RPC checks, measure latency, and return deterministic verdicts with useful exit codes.

Solana Infra Doctor is an independent open-source developer tool. It does not include token, NFT, airdrop, governance, points, marketplace, dashboard, cloud, or speculative crypto mechanics.

## Why It Exists

A Solana RPC endpoint can respond to a basic network request while still being unsuitable for production use. Developers need a fast way to verify basic RPC usability from their own environment before wiring an endpoint into applications, bots, wallets, or infrastructure jobs.

## Install From Source

```bash
git clone https://github.com/satyakwok/solana-infra-doctor.git
cd solana-infra-doctor
cargo build --release
```

The binary is built at:

```bash
./target/release/sol-doctor
```

## Usage

Check an RPC endpoint:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com
```

Emit JSON:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --json
```

Use a custom per-request timeout:

```bash
sol-doctor check --rpc https://api.mainnet-beta.solana.com --timeout-ms 3000
```

## Human-Readable Output Example

```text
Solana Infra Doctor
===================
RPC URL: https://api.mainnet-beta.solana.com/
Verdict: GOOD
Summary: all required RPC checks succeeded
Average latency: 120ms

Checks:
- getHealth      OK      80ms  health is ok
- getVersion     OK     110ms  solana-core 2.x.x
- getGenesisHash OK     130ms  5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp
- getSlot        OK     170ms  slot 123456789
```

## JSON Output Example

```json
{
  "verdict": "GOOD",
  "rpc_url": "https://api.mainnet-beta.solana.com/",
  "summary": "all required RPC checks succeeded",
  "average_latency_ms": 120,
  "checks": [
    {
      "method": "getHealth",
      "status": "success",
      "latency_ms": 80,
      "detail": "health is ok"
    },
    {
      "method": "getVersion",
      "status": "success",
      "latency_ms": 110,
      "detail": "solana-core 2.x.x"
    },
    {
      "method": "getGenesisHash",
      "status": "success",
      "latency_ms": 130,
      "detail": "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"
    },
    {
      "method": "getSlot",
      "status": "success",
      "latency_ms": 170,
      "detail": "slot 123456789"
    }
  ]
}
```

## Verdicts

- `GOOD`: all required RPC checks succeed and average latency is acceptable.
- `WARNING`: the RPC is reachable, but one non-critical check fails or average latency is elevated.
- `BAD`: the RPC URL is invalid, the endpoint cannot be reached, required RPC calls fail, repeated timeouts occur, or average latency is too high.
- `UNKNOWN`: there is not enough data to produce a reliable verdict.

Latency thresholds for v0.1:

- `GOOD`: average latency is less than or equal to 500ms.
- `WARNING`: average latency is greater than 500ms and less than or equal to 1500ms.
- `BAD`: average latency is greater than 1500ms or repeated timeouts occur.

## Exit Codes

- `0`: `GOOD`
- `1`: `WARNING`
- `2`: `BAD`
- `3`: internal or unexpected error

## Roadmap

- Broader RPC method coverage for production readiness checks.
- Optional cluster expectation checks.
- Better diagnostics for rate limiting and degraded providers.
- Endpoint comparison mode.
- Historical output formats suitable for CI and local automation.

WebSocket checks, Token Program checks, Token-2022 checks, dashboards, hosted services, and exporter modes are intentionally out of scope for v0.1.

## Disclaimer

Solana Infra Doctor is an independent open-source tool and is not affiliated with or endorsed by Solana Foundation.
