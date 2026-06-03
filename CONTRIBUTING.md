# Contributing

Thanks for your interest in Solana Infra Doctor.

## Development

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Keep v0.1 changes focused on the Rust CLI foundation and direct Solana JSON-RPC diagnostics. Avoid adding Solana SDK dependencies unless there is a clear need and the tradeoff is documented.

## Project Scope

This project is an independent developer tool. Do not add token, NFT, airdrop, governance, points, marketplace, hosted dashboard, cloud service, or speculative crypto mechanics.

## Pull Requests

- Keep changes small and reviewable.
- Include tests for behavior changes.
- Avoid logging sensitive RPC URLs, credentials, or query strings.
- Keep user-facing error messages clear and concise.
