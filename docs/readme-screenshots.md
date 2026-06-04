# README Screenshots

The CLI screenshots in the README (`docs/images/cli/{check,compare,ws}.png`) are
generated from **real `sol-doctor` runs against public Solana endpoints**. They
are not mockups, and the values are not hardcoded.

## What they are (and are not)

- **Real, live runs.** Each image is a real execution captured with color
  enabled. The latency, slot numbers, and slot lag are actual measurements.
- **Not mockups.** No image is hand-edited or assembled from fake data.
- **Not provider endorsements.** Showing an endpoint in a screenshot is not a
  recommendation or a benchmark of that provider.
- **Snapshots, not guarantees.** Values vary by time, region, this VPS's network
  egress, and endpoint conditions. A snapshot is not an SLA or uptime guarantee.

## Rules for regenerating them

- **Public endpoints only.** Use endpoints that need no credentials
  (`https://api.mainnet-beta.solana.com`, `https://solana-rpc.publicnode.com`).
- **Never use a private endpoint** or one that requires an API key, token, or
  Basic Auth — these images live in a public repository.
- **No secrets, ever.** URLs are redacted by the CLI, but do not point the tool
  at a URL containing a secret in the first place.
- For `compare`, both endpoints must be on the **same Solana network** (same
  genesis hash). If one is unavailable or rate-limited, substitute another
  verified public mainnet endpoint — do not fake the output.

## How to regenerate

Screenshots are refreshed **manually** (never in CI) with:

```bash
./scripts/capture-readme-screenshots.sh
```

The script builds the release binary, verifies its version, requires
[`freeze`](https://github.com/charmbracelet/freeze), runs each command live with
`--color always`, and writes `docs/images/cli/*.png`.

The exact commands captured are:

```bash
sol-doctor check   --rpc https://api.mainnet-beta.solana.com --color always
sol-doctor compare --rpc https://api.mainnet-beta.solana.com \
                   --rpc https://solana-rpc.publicnode.com --profile bot --color always
sol-doctor ws      --rpc https://api.mainnet-beta.solana.com --color always
```

## `freeze` is documentation tooling, not a dependency

`freeze` is used only to render these images locally. It is **not** a runtime or
build dependency of the crate: it is not in `Cargo.toml`, no `freeze` binary or
user config is committed, and nothing in the published crate depends on it. Install
it locally, outside the repository:

```bash
go install github.com/charmbracelet/freeze@v0.2.2
export PATH="$HOME/go/bin:$PATH"
```

The generated PNGs are excluded from the published crate (see `exclude` in
`Cargo.toml`) so they do not bloat the package.
