# Your first contribution

A 10-minute path from clone to PR. No deep Solana or Rust-internals knowledge needed.

## 1. Set up

```bash
git clone https://github.com/satyakwok/solana-infra-doctor
cd solana-infra-doctor
cargo build
```

## 2. See it work (real, public endpoint)

```bash
cargo run -- check --rpc https://api.mainnet-beta.solana.com --verbose
```

You should see a readiness panel with a `GOOD` / `WARNING` / `BAD` verdict.

## 3. Pick something small

- Browse [seed issues](seed-issues.md) — start with anything labeled
  **good first issue**.
- Or fix a typo / unclear sentence you hit in the README or `--help`.

## 4. Make the change on a branch

```bash
git checkout -b my-change
# edit files
```

## 5. Run the gates (the same ones CI runs)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

All three must pass. If you touched logic, add or update a test in `tests/`
(use the mock servers — no live network in tests) and keep coverage ≥ 95%.

## 6. Open the PR

Push your branch and open a PR. The template will ask you to confirm the gates and
to keep the diff small. One logical change per PR.

## Safety (always)

Never paste or commit a private RPC/gRPC URL, API key, or token — in code, tests,
examples, issues, or PRs. Use public endpoints (`api.mainnet-beta.solana.com`,
`api.devnet.solana.com`, `solana-rpc.publicnode.com`). For gRPC, tokens are read
only from an env var via `--x-token-env`.

## Where to go next

- [Add a new RPC check](add-a-check.md)
- [Add an example / terminal output](add-a-report-example.md)
- [Add a redaction-safe real-run report](real-run-evidence.md)
