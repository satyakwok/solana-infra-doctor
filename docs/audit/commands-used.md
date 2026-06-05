# Commands used for real-run evidence

All commands were run with `--color never` for byte-stable capture and a network
`timeout`. Public endpoints only; no credentials. Re-run any of these to reproduce.

```bash
BIN=sol-doctor   # or ./target/release/sol-doctor

# HTTP check
$BIN check --rpc https://api.mainnet-beta.solana.com --verbose
$BIN check --rpc https://api.devnet.solana.com --verbose
$BIN check --rpc https://api.mainnet-beta.solana.com --data --verbose
$BIN check --rpc https://api.mainnet-beta.solana.com --json
$BIN check --rpc https://api.mainnet-beta.solana.com --samples 10

# HTTP compare (public endpoints)
$BIN compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com --profile bot --verbose
$BIN compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com --profile indexer --data --verbose
$BIN compare \
  --rpc https://api.mainnet-beta.solana.com \
  --rpc https://solana-rpc.publicnode.com --profile indexer --data \
  --report report.md

# WebSocket
$BIN ws --rpc https://api.mainnet-beta.solana.com --verbose
$BIN ws --rpc https://api.mainnet-beta.solana.com --subscription logs --verbose

# Yellowstone gRPC — BLOCKED without a private endpoint + token. Run your own:
export YELLOWSTONE_X_TOKEN="your-token"
$BIN grpc check --grpc https://your-yellowstone-endpoint \
  --x-token-env YELLOWSTONE_X_TOKEN
$BIN grpc compare \
  --grpc https://endpoint-a --x-token-env YELLOWSTONE_A_TOKEN \
  --grpc https://endpoint-b --x-token-env YELLOWSTONE_B_TOKEN --profile latency
```
