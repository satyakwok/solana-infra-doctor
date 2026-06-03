# Changelog

## 0.1.0 - Unreleased

- Add initial Rust CLI foundation.
- Add `sol-doctor check --rpc <RPC_URL>`.
- Validate RPC URLs.
- Run raw HTTP JSON-RPC checks for `getHealth`, `getVersion`, `getGenesisHash`, and `getSlot`.
- Measure per-method latency and average latency.
- Add human-readable and JSON output.
- Add deterministic verdicts and exit codes.
- Add CI, documentation, and tests.
