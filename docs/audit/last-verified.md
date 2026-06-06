# Last verified

- **Verified (UTC):** 2026-06-05T20:38:25Z
- **Tool version:** sol-doctor 0.13.0
- **Git commit:** b3f771f
- **Endpoints used (public, no credentials):**
  - HTTP RPC: `https://api.mainnet-beta.solana.com`, `https://api.devnet.solana.com`, `https://solana-rpc.publicnode.com`
  - WebSocket: `wss://api.mainnet-beta.solana.com` (derived from the HTTP URL)
- **Yellowstone gRPC `check`:** validated against a real credentialed **Tatum mainnet**
  endpoint (`https://solana-mainnet-grpc.gateway.tatum.io`, `x-token` via
  `--x-token-env`). Result: **degraded** (auth + live `GetSlot` passed; slot stream
  not ready; some unary calls rate-limited 429). See [`grpc-real-run.md`](grpc-real-run.md).
  No token committed.
- **Yellowstone gRPC `compare`:** blocked — needs multiple credentialed endpoints.

Re-running the commands in [`commands-used.md`](commands-used.md) regenerates the
evidence under [`examples/terminal/`](../../examples/terminal/) and
[`examples/reports/`](../../examples/reports/). Values vary by time, region, and
endpoint conditions.
